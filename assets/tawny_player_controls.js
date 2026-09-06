(function () {
  "use strict";

  const controllers = new WeakMap();
  const pendingMetadata = new WeakMap();
  const speedSteps = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];
  const quickSpeedSteps = [1, 1.25, 1.5, 2];

  function formatTime(value) {
    if (!Number.isFinite(value) || value < 0) return "0:00";
    const seconds = Math.floor(value % 60);
    const minutes = Math.floor(value / 60) % 60;
    const hours = Math.floor(value / 3600);
    const tail = `${minutes}:${String(seconds).padStart(2, "0")}`;
    return hours > 0
      ? `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`
      : tail;
  }

  function fullscreenElement() {
    return document.fullscreenElement || document.webkitFullscreenElement || null;
  }

  function toggleFullscreen(root) {
    if (fullscreenElement()) {
      const exit = document.exitFullscreen || document.webkitExitFullscreen;
      if (screen.orientation && typeof screen.orientation.unlock === "function") {
        try {
          screen.orientation.unlock();
        } catch (_) {}
      }
      root.querySelector("[data-player-native-orientation-unlock]")?.click();
      return exit ? exit.call(document) : undefined;
    }
    return enterFullscreenFor(root);
  }

  /// Go fullscreen and, when enabled, rotate horizontal video to landscape.
  /// Orientation locking only exists on
  /// mobile and rejects when unsupported, so failures are ignored.
  ///
  /// iOS Safari never implemented fullscreen on ordinary elements, so the video
  /// element's own presentation mode is the fallback there. Without it the
  /// button did nothing at all on a handset.
  function enterFullscreenFor(root) {
    const media = root.querySelector("video");
    const horizontal = media && media.videoWidth > media.videoHeight;
    const autoLandscape = root.dataset.autoLandscape !== "false";
    const request =
      root.requestFullscreen || root.webkitRequestFullscreen || root.webkitRequestFullScreen;
    const started = request
      ? Promise.resolve(request.call(root, { navigationUI: "hide" })).catch(() =>
          Promise.resolve(request.call(root)).catch(() => Promise.reject()),
        )
      : Promise.reject();
    return started
      .then(() => {
        if (horizontal && autoLandscape) {
          root.querySelector("[data-player-native-landscape]")?.click();
          const orientation = screen.orientation;
          if (orientation && typeof orientation.lock === "function") {
            return orientation.lock("landscape").catch(() => {});
          }
        }
      })
      .catch(() => {
        if (media && typeof media.webkitEnterFullscreen === "function") {
          try {
            media.webkitEnterFullscreen();
          } catch (_) {}
        }
      });
  }

  function attach(video) {
    if (!video) return;
    const root = video.closest("#tawny-player");
    const controls = root && root.querySelector(".tawny-player-controls");
    if (!root || !controls) return;

    const existing = controllers.get(video);
    if (existing) existing.destroy();

    const abort = new AbortController();
    const signal = abort.signal;
    const progress = controls.querySelector("[data-player-progress]");
    const chapterMarkers = controls.querySelector("[data-player-chapter-markers]");
    const chapterLabel = controls.querySelector("[data-player-chapter-label]");
    const seekPreview = controls.querySelector("[data-player-seek-preview]");
    const previewImage = controls.querySelector("[data-player-preview-image]");
    const previewChapter = controls.querySelector("[data-player-preview-chapter]");
    const previewTime = controls.querySelector("[data-player-preview-time]");
    const currentTime = controls.querySelector("[data-player-current-time]");
    const duration = controls.querySelector("[data-player-duration]");
    const speedLabel = controls.querySelector("[data-player-speed-label]");
    const optionsMenu = controls.querySelector("[data-player-options-menu]");
    const qualityOptions = controls.querySelector("[data-player-quality-options]");
    const playButton = controls.querySelector('[data-player-action="toggle"]');
    const muteButton = controls.querySelector('[data-player-action="mute"]');
    const captionsButton = controls.querySelector('[data-player-action="captions"]');
    const fullscreenButton = controls.querySelector('[data-player-action="fullscreen"]');
    const buffering = root.querySelector(".player-buffering-indicator");
    const feedbackLeft = root.querySelector(".player-seek-feedback-left");
    const feedbackRight = root.querySelector(".player-seek-feedback-right");
    let hideTimer = null;
    let feedbackTimer = null;
    let scrubbing = false;
    let resumeAfterScrub = false;
    let animationFrame = null;
    let chosenHeight = null;
    let captions = [];
    let selectedCaption = null;
    let captionsEnabled = false;
    let chapters = [];
    let previewFrames = null;
    let playbackIntent = !video.paused && !video.ended;
    let playbackRecoveryTimer = null;
    let pipTransition = false;
    let pipTransitionTimer = null;

    const listen = (target, name, handler, options) =>
      target.addEventListener(name, handler, { ...options, signal });

    function setPlaybackIntent(shouldPlay) {
      playbackIntent = Boolean(shouldPlay);
      // The Android PiP actions execute outside this closure. Keeping the
      // intent on the element lets their play/pause command participate in the
      // same lifecycle recovery without mistaking an explicit Pause for a
      // transient Activity pause.
      video.__tawnyPlaybackIntent = playbackIntent;
    }

    function beginPipTransition() {
      pipTransition = true;
      if (pipTransitionTimer) clearTimeout(pipTransitionTimer);
      pipTransitionTimer = setTimeout(() => {
        pipTransition = false;
        pipTransitionTimer = null;
      }, 1400);
    }

    function recoverIntendedPlayback(delay = 0) {
      if (!playbackIntent || video.ended) return;
      if (playbackRecoveryTimer) clearTimeout(playbackRecoveryTimer);
      playbackRecoveryTimer = setTimeout(() => {
        playbackRecoveryTimer = null;
        if (playbackIntent && video.paused && !video.ended) {
          video.play().catch(() => {});
        }
      }, delay);
    }

    function showControls(permanent) {
      controls.classList.add("controls-visible");
      root.classList.add("player-controls-visible");
      if (hideTimer) clearTimeout(hideTimer);
      if (!permanent && !video.paused && optionsMenu && !optionsMenu.hidden) return;
      if (!permanent && !video.paused) {
        hideTimer = setTimeout(() => {
          if (!scrubbing && (optionsMenu?.hidden ?? true)) {
            controls.classList.remove("controls-visible");
            root.classList.remove("player-controls-visible");
          }
        }, 2800);
      }
    }

    function scheduleHide() {
      showControls(false);
    }

    function updatePlaybackState() {
      const playing = !video.paused && !video.ended;
      controls.classList.toggle("is-playing", playing);
      playButton?.setAttribute("aria-label", playing ? "Pause" : "Play");
      if (video.paused || video.ended) showControls(true);
      else scheduleHide();
    }

    function bufferedEnd() {
      let end = 0;
      for (let index = 0; index < video.buffered.length; index += 1) {
        end = Math.max(end, video.buffered.end(index));
      }
      return end;
    }

    function updateChapterLabel(position) {
      if (!chapterLabel) return;
      const chapter = chapterAt(position);
      const title = chapter?.title || "";
      chapterLabel.hidden = !title;
      if (chapterLabel.textContent !== title) chapterLabel.textContent = title;
    }

    function updateTimeline() {
      const length = Number.isFinite(video.duration) ? video.duration : 0;
      const position = Number.isFinite(video.currentTime) ? video.currentTime : 0;
      if (currentTime) currentTime.textContent = formatTime(position);
      updateChapterLabel(position);
      // The markers are appended imperatively into an element the framework
      // owns, so any re-render of that subtree silently drops them. Rebuilding
      // when they have gone missing is what keeps them on screen.
      if (chapterMarkers && chapters.length > 1 && length > 0 && !chapterMarkers.firstChild) {
        renderChapters();
      }
      if (duration) duration.textContent = formatTime(length);
      if (!progress || scrubbing) return;
      const played = length > 0 ? Math.min(100, (position / length) * 100) : 0;
      const buffered = length > 0 ? Math.min(100, (bufferedEnd() / length) * 100) : 0;
      progress.value = String(Math.round(played * 100));
      progress.style.setProperty("--player-progress", `${played}%`);
      progress.style.setProperty("--player-buffered", `${Math.max(played, buffered)}%`);
      progress.setAttribute("aria-valuetext", `${formatTime(position)} of ${formatTime(length)}`);
    }

    function effectivePlaybackRate() {
      const rate = Number(video.playbackRate);
      if (Number.isFinite(rate) && rate > 0) return rate;
      const fallback = Number(video.defaultPlaybackRate);
      return Number.isFinite(fallback) && fallback > 0 ? fallback : 1;
    }

    function updateSpeed() {
      const rate = effectivePlaybackRate();
      const value = `${Number(rate.toFixed(2))}×`;
      if (speedLabel) speedLabel.textContent = value;
      controls.querySelectorAll("[data-player-speed]").forEach((button) => {
        button.classList.toggle(
          "selected",
          Math.abs(Number(button.dataset.playerSpeed) - rate) < 0.01,
        );
      });
    }

    function updateMuted() {
      const muted = video.muted || video.volume === 0;
      controls.classList.toggle("is-muted", muted);
      muteButton?.setAttribute("aria-label", muted ? "Unmute" : "Mute");
    }

    function updateCaptions() {
      const tracks = Array.from(video.textTracks || []);
      const selected = tracks.findIndex((track) => track.mode === "showing");
      captionsButton?.classList.toggle("selected", selected >= 0);
      captionsButton?.toggleAttribute("disabled", tracks.length === 0);
      if (captionsButton) {
        captionsButton.title =
          selected >= 0 ? `Captions: ${tracks[selected].label || "On"}` : "Captions off";
      }
    }

    function applyCaptionState() {
      const tracks = Array.from(video.textTracks || []);
      const validSelection =
        Number.isInteger(selectedCaption) &&
        selectedCaption >= 0 &&
        selectedCaption < tracks.length;
      tracks.forEach((track, index) => {
        track.mode =
          captionsEnabled && validSelection && index === selectedCaption ? "showing" : "disabled";
      });
      updateCaptions();
    }

    function chapterAt(position) {
      let active = null;
      for (const chapter of chapters) {
        if (Number(chapter.start_seconds) <= position) active = chapter;
        else break;
      }
      return active;
    }

    function renderChapters() {
      if (!chapterMarkers) return;
      chapterMarkers.replaceChildren();
      const length = Number.isFinite(video.duration) ? video.duration : 0;
      if (length <= 0 || chapters.length < 2) return;
      for (const chapter of chapters.slice(1)) {
        const marker = document.createElement("span");
        marker.className = "player-chapter-marker";
        marker.style.left = `${Math.min(100, (Number(chapter.start_seconds) / length) * 100)}%`;
        marker.title = chapter.title || formatTime(Number(chapter.start_seconds));
        chapterMarkers.appendChild(marker);
      }
    }

    function previewFrame(position) {
      const data = previewFrames;
      if (!data || !data.page_urls?.length || !data.duration_per_frame_ms) return null;
      const count = Math.max(1, Number(data.total_count) || 1);
      const frame = Math.min(count - 1, Math.floor((position * 1000) / data.duration_per_frame_ms));
      const columns = Math.max(1, Number(data.frames_per_page_x) || 1);
      const rows = Math.max(1, Number(data.frames_per_page_y) || 1);
      const perPage = columns * rows;
      const page = Math.min(data.page_urls.length - 1, Math.floor(frame / perPage));
      const cell = frame % perPage;
      return {
        url: data.page_urls[page],
        columns,
        rows,
        column: cell % columns,
        row: Math.floor(cell / columns),
      };
    }

    function showScrubPreview(position) {
      if (!seekPreview || !progress) return;
      const length = Number.isFinite(video.duration) ? video.duration : 0;
      const percent = length > 0 ? Math.min(100, Math.max(0, (position / length) * 100)) : 0;
      seekPreview.hidden = false;
      seekPreview.style.left = `clamp(5rem, ${percent}%, calc(100% - 5rem))`;
      if (previewTime) previewTime.textContent = formatTime(position);
      const chapter = chapterAt(position);
      if (previewChapter) {
        previewChapter.textContent = chapter?.title || "";
        previewChapter.hidden = !chapter?.title;
      }
      const frame = previewFrame(position);
      if (previewImage) {
        const url = frame?.url || root.dataset.thumbnail || video.poster || "";
        previewImage.style.backgroundImage = url ? `url(${JSON.stringify(url)})` : "none";
        if (frame) {
          previewImage.style.backgroundSize = `${frame.columns * 100}% ${frame.rows * 100}%`;
          const x = frame.columns > 1 ? (frame.column / (frame.columns - 1)) * 100 : 0;
          const y = frame.rows > 1 ? (frame.row / (frame.rows - 1)) * 100 : 0;
          previewImage.style.backgroundPosition = `${x}% ${y}%`;
        } else {
          previewImage.style.backgroundSize = "cover";
          previewImage.style.backgroundPosition = "center";
        }
      }
    }

    function hideScrubPreview() {
      if (seekPreview) seekPreview.hidden = true;
    }

    function setMetadata(metadata) {
      const next = metadata || {};
      const nextCaptions = next.captions || [];
      captions = nextCaptions;
      selectedCaption = Number.isInteger(next.selectedCaption) ? next.selectedCaption : null;
      captionsEnabled = Boolean(next.captionsEnabled);
      chapters = (next.chapters || [])
        .slice()
        .sort((left, right) => Number(left.start_seconds) - Number(right.start_seconds));
      previewFrames = next.previewFrames || null;
      applyCaptionState();
      requestAnimationFrame(applyCaptionState);
      setTimeout(applyCaptionState, 100);
      renderChapters();
    }

    function showSeekFeedback(direction) {
      const feedback = direction < 0 ? feedbackLeft : feedbackRight;
      if (!feedback) return;
      feedback.classList.remove("show-feedback");
      void feedback.offsetWidth;
      feedback.classList.add("show-feedback");
      if (feedbackTimer) clearTimeout(feedbackTimer);
      feedbackTimer = setTimeout(() => feedback.classList.remove("show-feedback"), 650);
    }

    function seekBy(amount) {
      const length = Number.isFinite(video.duration) ? video.duration : Infinity;
      video.currentTime = Math.max(0, Math.min(length, video.currentTime + amount));
      showSeekFeedback(amount < 0 ? -1 : 1);
      updateTimeline();
      showControls(false);
    }

    function setSpeed(speed) {
      if (!Number.isFinite(speed) || speed <= 0) return;
      video.playbackRate = speed;
      video.defaultPlaybackRate = speed;
      updateSpeed();
    }

    function cycleQuickSpeed() {
      const current = quickSpeedSteps.findIndex(
        (speed) => Math.abs(speed - effectivePlaybackRate()) < 0.01,
      );
      setSpeed(quickSpeedSteps[(current + 1) % quickSpeedSteps.length]);
    }

    function toggleOptions() {
      if (!optionsMenu) return;
      optionsMenu.hidden = !optionsMenu.hidden;
      controls.classList.toggle("options-open", !optionsMenu.hidden);
      showControls(!optionsMenu.hidden);
      if (!optionsMenu.hidden) refreshQualities();
    }

    function closeOptions() {
      if (!optionsMenu || optionsMenu.hidden) return;
      optionsMenu.hidden = true;
      controls.classList.remove("options-open");
    }

    function refreshQualities() {
      if (!qualityOptions || !window.TawnyTransport?.qualityState) return;
      const state = window.TawnyTransport.qualityState(video);
      if (!state) return;
      chosenHeight = state.auto ? null : state.selectedHeight;
      qualityOptions.replaceChildren();
      const choices = [
        { value: null, label: state.activeHeight ? `Auto · ${state.activeHeight}p` : "Auto" },
      ].concat(state.heights.map((height) => ({ value: height, label: `${height}p` })));
      for (const choice of choices) {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "player-option-button";
        button.dataset.playerQuality = choice.value === null ? "auto" : String(choice.value);
        button.textContent = choice.label;
        button.classList.toggle("selected", choice.value === chosenHeight);
        qualityOptions.appendChild(button);
      }
    }

    async function runAction(action) {
      switch (action) {
        case "toggle":
          if (video.paused) {
            setPlaybackIntent(true);
            await video.play().catch(() => {});
          } else {
            setPlaybackIntent(false);
            video.pause();
          }
          break;
        case "back":
          if (fullscreenElement()) {
            await Promise.resolve(toggleFullscreen(root)).catch(() => {});
            // Let Android finish its fullscreen/orientation transition before
            // navigating the player sheet away. Doing both in the same frame
            // can make the system restore the just-removed fullscreen view.
            await new Promise((resolve) => requestAnimationFrame(resolve));
          }
          root.querySelector("[data-player-minimize]")?.click();
          break;
        case "rewind":
          seekBy(-10);
          break;
        case "forward":
          seekBy(10);
          break;
        case "mute":
          video.muted = !video.muted;
          updateMuted();
          break;
        case "captions":
          controls.querySelector("[data-player-caption-toggle]")?.click();
          break;
        case "chapters":
          (
            controls.querySelector("[data-player-chapters-open]") ||
            root.querySelector("[data-player-chapters-open]") ||
            document.querySelector("[data-player-chapters-open]")
          )?.click();
          break;
        case "speed":
          cycleQuickSpeed();
          break;
        case "settings":
          toggleOptions();
          break;
        case "pip": {
          let pipChanged = false;
          try {
            if (document.pictureInPictureElement) {
              await document.exitPictureInPicture();
              pipChanged = true;
            } else if (video.requestPictureInPicture) {
              await video.requestPictureInPicture();
              pipChanged = true;
            } else if (video.webkitSupportsPresentationMode) {
              video.webkitSetPresentationMode("picture-in-picture");
              pipChanged = true;
            }
          } catch (_) {}
          if (!pipChanged) {
            beginPipTransition();
            setPlaybackIntent(!video.paused && !video.ended);
            const portrait = video.videoHeight > video.videoWidth;
            const selector = portrait
              ? "[data-player-native-pip-portrait]"
              : "[data-player-native-pip-landscape]";
            root.querySelector(selector)?.click();
          }
          break;
        }
        case "fullscreen":
          await Promise.resolve(toggleFullscreen(root)).catch(() => {});
          break;
      }
      if (action !== "settings") scheduleHide();
    }

    listen(controls, "click", (event) => {
      const speed = event.target.closest("[data-player-speed]");
      if (speed) {
        setSpeed(Number(speed.dataset.playerSpeed));
        return;
      }
      const quality = event.target.closest("[data-player-quality]");
      if (quality && window.TawnyTransport?.setQuality) {
        const value = quality.dataset.playerQuality;
        window.TawnyTransport.setQuality(video, value === "auto" ? null : Number(value));
        refreshQualities();
        return;
      }
      const button = event.target.closest("[data-player-action]");
      if (button) runAction(button.dataset.playerAction);
    });

    if (progress) {
      // Ending a scrub has to be idempotent and reachable from every exit
      // route. A pointerdown that never changes the value (pressing straight
      // on the thumb) emits no `input` and no `change`, so relying on `change`
      // alone would leave the player paused with a frozen timeline, since
      // updateTimeline() bails out while `scrubbing` is set.
      const positionScrubAt = (clientX) => {
        const bounds = progress.getBoundingClientRect();
        if (!bounds.width) return;
        const ratio = Math.max(0, Math.min(1, (clientX - bounds.left) / bounds.width));
        progress.value = String(Math.round(ratio * 10000));
        const length = Number.isFinite(video.duration) ? video.duration : 0;
        const position = ratio * length;
        if (currentTime) currentTime.textContent = formatTime(position);
        progress.style.setProperty("--player-progress", `${ratio * 100}%`);
        progress.setAttribute("aria-valuetext", `${formatTime(position)} of ${formatTime(length)}`);
        showScrubPreview(position);
      };

      const beginScrub = () => {
        if (scrubbing) return;
        scrubbing = true;
        resumeAfterScrub = !video.paused && !video.ended;
        if (resumeAfterScrub) video.pause();
        showControls(true);
      };

      const finishScrub = (commit) => {
        if (!scrubbing) return;
        scrubbing = false;
        const length = Number.isFinite(video.duration) ? video.duration : 0;
        if (commit && length > 0) {
          video.currentTime = (Number(progress.value) / 10000) * length;
        }
        hideScrubPreview();
        if (resumeAfterScrub) video.play().catch(() => {});
        resumeAfterScrub = false;
        updateTimeline();
        scheduleHide();
      };

      listen(progress, "pointerdown", (event) => {
        beginScrub();
        try {
          progress.setPointerCapture(event.pointerId);
        } catch (_) {}
        positionScrubAt(event.clientX);
      });
      listen(progress, "pointermove", (event) => {
        if (scrubbing) positionScrubAt(event.clientX);
      });
      listen(progress, "input", () => {
        // Android WebView can hand a range drag directly to the native range
        // control without first surfacing pointerdown. Treat the first input
        // as the start so the clock freezes at the preview position and the
        // media still pauses throughout the gesture.
        beginScrub();
        const length = Number.isFinite(video.duration) ? video.duration : 0;
        const position = (Number(progress.value) / 10000) * length;
        if (currentTime) currentTime.textContent = formatTime(position);
        progress.style.setProperty("--player-progress", `${Number(progress.value) / 100}%`);
        showScrubPreview(position);
      });
      listen(progress, "change", () => finishScrub(true));
      listen(progress, "pointerup", () => finishScrub(true));
      listen(progress, "pointercancel", () => finishScrub(false));
      // Range inputs capture the pointer, but a capture lost to a scroll
      // gesture or a release over another element still has to settle.
      listen(progress, "lostpointercapture", () => finishScrub(true));
      listen(window, "pointerup", () => finishScrub(true));
      listen(progress, "keyup", () => finishScrub(true));
      listen(progress, "blur", () => finishScrub(true));
    }

    // Tapping anywhere on the video toggles playback — unless the pointer just
    // travelled far enough to be a swipe, in which case the gesture owns it.
    listen(video, "click", () => {
      closeOptions();
      if (swallowNextClick) {
        swallowNextClick = false;
        return;
      }
      runAction("toggle");
      showControls(false);
    });

    // Vertical swipes: up enters fullscreen, down leaves it, and a downward
    // swipe outside fullscreen shrinks the player to the mini bar.
    let gestureStart = null;
    let swallowNextClick = false;
    const SWIPE_DISTANCE = 55;

    const trackGesture = (event) => {
      if (!gestureStart) return;
      const dy = event.clientY - gestureStart.y;
      const dx = event.clientX - gestureStart.x;
      if (!gestureStart.committed) {
        // Claim the gesture only once it is clearly vertical. Until then the
        // page keeps its normal scrolling.
        if (Math.abs(dy) < 10 || Math.abs(dy) <= Math.abs(dx)) return;
        gestureStart.committed = true;
        try {
          root.setPointerCapture(event.pointerId);
        } catch (_) {}
      }
      // Follow the finger the way YouTube does: swiping up grows the video
      // toward its fullscreen size so the gesture feels attached to it.
      const progress = Math.min(1, Math.max(0, -dy / 260));
      root.style.setProperty("--player-swipe-progress", progress.toFixed(3));
      root.classList.toggle("player-swiping", progress > 0);
      // Suppress the page's own scrolling while we own the gesture.
      if (event.cancelable) event.preventDefault();
    };

    const resetGestureVisuals = () => {
      root.style.removeProperty("--player-swipe-progress");
      root.classList.remove("player-swiping");
    };

    listen(root, "pointerdown", (event) => {
      swallowNextClick = false;
      if (
        event.target.closest("[data-player-action], [data-player-progress], .player-options-menu")
      ) {
        gestureStart = null;
        return;
      }
      gestureStart = {
        x: event.clientX,
        y: event.clientY,
        at: Date.now(),
        committed: event.pointerType === "mouse",
      };
      if (gestureStart.committed) {
        // A mouse drag never competes with scrolling, so capture immediately;
        // a release outside the player still has to report back here.
        try {
          root.setPointerCapture(event.pointerId);
        } catch (_) {}
      }
    });

    listen(root, "pointermove", trackGesture);

    const endGesture = (event) => {
      if (!gestureStart) return;
      const dx = event.clientX - gestureStart.x;
      const dy = event.clientY - gestureStart.y;
      const elapsed = Date.now() - gestureStart.at;
      gestureStart = null;
      resetGestureVisuals();
      try {
        root.releasePointerCapture(event.pointerId);
      } catch (_) {}
      // Deliberate, mostly-vertical, and quick enough to be a flick.
      if (elapsed > 800 || Math.abs(dy) < SWIPE_DISTANCE || Math.abs(dy) <= Math.abs(dx)) {
        return;
      }
      // A swipe is not a tap: keep the click that follows from toggling play.
      swallowNextClick = true;
      const fullscreen = document.fullscreenElement === root;
      if (dy < 0) {
        // Requested straight from the handler: fullscreen needs the user
        // activation this event carries, and awaiting anything first loses it.
        if (!fullscreen) enterFullscreenFor(root);
      } else if (fullscreen) {
        document.exitFullscreen && document.exitFullscreen();
      } else {
        controls.querySelector("[data-player-minimize]")?.click();
      }
    };

    listen(root, "pointerup", endGesture);
    listen(root, "pointercancel", () => {
      gestureStart = null;
      resetGestureVisuals();
    });
    listen(video, "dblclick", (event) => {
      const bounds = video.getBoundingClientRect();
      const position = (event.clientX - bounds.left) / bounds.width;
      if (position < 0.4) seekBy(-10);
      else if (position > 0.6) seekBy(10);
      else Promise.resolve(toggleFullscreen(root)).catch(() => {});
    });
    listen(root, "pointermove", () => showControls(false));
    listen(root, "pointerleave", scheduleHide);
    listen(root, "keydown", (event) => {
      if (event.target instanceof HTMLInputElement) return;
      const key = event.key.toLowerCase();
      if (key === "escape" && optionsMenu && !optionsMenu.hidden) {
        event.preventDefault();
        closeOptions();
        return;
      }
      const handled = [" ", "k", "j", "l", "arrowleft", "arrowright", "m", "f", "c"];
      if (!handled.includes(key)) return;
      event.preventDefault();
      if (key === " " || key === "k") runAction("toggle");
      else if (key === "j" || key === "arrowleft") seekBy(-10);
      else if (key === "l" || key === "arrowright") seekBy(10);
      else if (key === "m") runAction("mute");
      else if (key === "f") runAction("fullscreen");
      else if (key === "c") runAction("captions");
    });

    listen(video, "play", () => {
      setPlaybackIntent(true);
      updatePlaybackState();
      root.querySelector("[data-player-native-playback-start]")?.click();
    });
    listen(video, "pause", () => {
      updatePlaybackState();
      if (scrubbing) return;
      // Native PiP controls write their intent directly onto the media
      // element before pausing. Read it first so an explicit PiP Pause is not
      // immediately undone by lifecycle recovery.
      if (video.__tawnyPlaybackIntent === false) playbackIntent = false;
      if (playbackIntent) {
        // Shaka can briefly pause the media element while recovering a range
        // request or rebuilding a SourceBuffer. That is not a user Pause, so
        // keep Android's audio session alive and let Shaka resume it. During an
        // actual lifecycle edge, actively ask Chromium to resume as well.
        if (pipTransition || document.visibilityState !== "visible") {
          recoverIntendedPlayback(80);
        }
        return;
      }
      root.querySelector("[data-player-native-playback-stop]")?.click();
    });
    listen(video, "ended", () => {
      setPlaybackIntent(false);
      updatePlaybackState();
      root.querySelector("[data-player-native-playback-stop]")?.click();
    });
    listen(window, "tawnynativepiprequest", beginPipTransition);
    listen(window, "tawnynativepictureinpicturechange", () => {
      // Entering and leaving PiP can each pause the WebView after the native
      // mode callback. Keep a short grace window around both edges.
      beginPipTransition();
      recoverIntendedPlayback(60);
    });
    listen(window, "tawnynativeplaybackresume", () => recoverIntendedPlayback(0));
    listen(video, "enterpictureinpicture", () => {
      beginPipTransition();
      recoverIntendedPlayback(0);
    });
    listen(video, "leavepictureinpicture", () => {
      beginPipTransition();
      recoverIntendedPlayback(0);
    });
    listen(video, "durationchange", updateTimeline);
    listen(video, "durationchange", renderChapters);
    listen(video, "progress", updateTimeline);
    listen(video, "ratechange", updateSpeed);
    listen(video, "volumechange", updateMuted);
    listen(video, "waiting", () => buffering?.classList.add("buffering-visible"));
    listen(video, "playing", () => buffering?.classList.remove("buffering-visible"));
    listen(video, "canplay", () => buffering?.classList.remove("buffering-visible"));
    listen(video, "loadedmetadata", updateCaptions);
    Array.from(video.querySelectorAll("track[data-tawny-caption]")).forEach((track) => {
      listen(track, "load", applyCaptionState);
    });
    listen(video, "tawnytransportchange", () => setTimeout(refreshQualities, 0));
    listen(video, "tawnyqualitychange", refreshQualities);
    const syncFullscreen = () => {
      // The iOS fallback puts the video element itself fullscreen, not the
      // root, so anything inside this player counts.
      const active = fullscreenElement();
      const fullscreen = Boolean(active && (active === root || root.contains(active)));
      if (!fullscreen) root.querySelector("[data-player-native-orientation-unlock]")?.click();
      root.classList.toggle("is-fullscreen", fullscreen);
      controls.classList.toggle("is-fullscreen", fullscreen);
      fullscreenButton?.setAttribute(
        "aria-label",
        fullscreen ? "Exit fullscreen" : "Enter fullscreen",
      );
      fullscreenButton?.setAttribute("title", fullscreen ? "Exit fullscreen" : "Fullscreen");
      showControls(false);
    };
    listen(document, "fullscreenchange", syncFullscreen);
    listen(document, "webkitfullscreenchange", syncFullscreen);
    listen(video, "webkitendfullscreen", syncFullscreen);

    root.tabIndex = 0;
    setPlaybackIntent(playbackIntent);
    updatePlaybackState();
    updateTimeline();
    updateSpeed();
    updateMuted();
    updateCaptions();
    setTimeout(refreshQualities, 0);
    const animateTimeline = () => {
      updateTimeline();
      animationFrame = requestAnimationFrame(animateTimeline);
    };
    animationFrame = requestAnimationFrame(animateTimeline);

    const controller = {
      setMetadata,
      destroy() {
        abort.abort();
        if (animationFrame) cancelAnimationFrame(animationFrame);
        if (hideTimer) clearTimeout(hideTimer);
        if (feedbackTimer) clearTimeout(feedbackTimer);
        if (playbackRecoveryTimer) clearTimeout(playbackRecoveryTimer);
        if (pipTransitionTimer) clearTimeout(pipTransitionTimer);
      },
    };
    controllers.set(video, controller);
    const pending = pendingMetadata.get(video);
    if (pending) {
      pendingMetadata.delete(video);
      controller.setMetadata(pending);
    }
  }

  function detach(video) {
    const controller = controllers.get(video);
    if (!controller) return;
    controller.destroy();
    controllers.delete(video);
  }

  function setMetadata(video, metadata) {
    const controller = controllers.get(video);
    if (controller) controller.setMetadata(metadata);
    else pendingMetadata.set(video, metadata);
  }

  window.TawnyPlayerControls = { attach, detach, setMetadata };
})();
