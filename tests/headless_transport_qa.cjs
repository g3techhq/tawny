const { spawn } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const WebSocket = require("ws");

const chrome = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "tawny-transport-"));
const port = 9223;
const target = process.argv[2] || "http://localhost:8080/watch/dQw4w9WgXcQ";
const requiredPlaybackSeconds = Number(process.argv[3] || 0);
const faultAtSeconds = Number(process.argv[4] || -1);
const browser = spawn(
  chrome,
  [
    "--headless=new",
    "--disable-gpu",
    "--no-first-run",
    "--disable-default-apps",
    "--autoplay-policy=no-user-gesture-required",
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`,
    target,
  ],
  { stdio: "ignore", windowsHide: true },
);

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function json(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Chrome returned ${response.status}`);
  return response.json();
}

async function pageTarget() {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      const pages = await json(`http://127.0.0.1:${port}/json/list`);
      const page = pages.find((candidate) => candidate.type === "page");
      if (page) return page;
    } catch (_) {}
    await delay(250);
  }
  throw new Error("Chrome debugging target did not start");
}

async function inspect() {
  const page = await pageTarget();
  const socket = new WebSocket(page.webSocketDebuggerUrl);
  const pending = new Map();
  const consoleMessages = [];
  const samples = [];
  const requests = new Map();
  const networkFailures = [];
  let id = 0;
  let faultInjected = false;

  socket.on("message", (bytes) => {
    const message = JSON.parse(bytes.toString());
    if (message.id && pending.has(message.id)) {
      const { resolve, reject } = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) reject(new Error(message.error.message));
      else resolve(message.result);
      return;
    }
    if (message.method === "Runtime.consoleAPICalled") {
      consoleMessages.push(
        message.params.args.map((argument) => argument.value || argument.description).join(" "),
      );
    }
    if (message.method === "Log.entryAdded") {
      consoleMessages.push(`${message.params.entry.level}: ${message.params.entry.text}`);
    }
    if (message.method === "Network.requestWillBeSent") {
      const request = message.params.request;
      if (request.url.includes("/api/v1/playback/")) {
        requests.set(message.params.requestId, {
          url: request.url,
          range: request.headers.Range || request.headers.range || null,
        });
      }
    }
    if (message.method === "Network.responseReceived") {
      const request = requests.get(message.params.requestId);
      if (request) {
        request.status = message.params.response.status;
        request.contentRange =
          message.params.response.headers["content-range"] ||
          message.params.response.headers["Content-Range"] ||
          null;
      }
    }
    if (message.method === "Network.loadingFailed") {
      const request = requests.get(message.params.requestId);
      if (request) {
        networkFailures.push({
          ...request,
          error: message.params.errorText,
          cancelled: message.params.canceled || false,
        });
      }
    }
    if (message.method === "Network.responseReceived" && message.params.response.status >= 400) {
      consoleMessages.push(
        `http ${message.params.response.status}: ${message.params.response.url}`,
      );
    }
    if (message.method === "Runtime.exceptionThrown") {
      consoleMessages.push(
        `exception: ${message.params.exceptionDetails.exception?.description || message.params.exceptionDetails.text}`,
      );
    }
  });

  await new Promise((resolve, reject) => {
    socket.once("open", resolve);
    socket.once("error", reject);
  });
  const command = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const commandId = ++id;
      pending.set(commandId, { resolve, reject });
      socket.send(JSON.stringify({ id: commandId, method, params }));
    });
  await command("Runtime.enable");
  await command("Log.enable");
  await command("Network.enable");

  let state = null;
  const maxAttempts = Math.max(60, Math.ceil((requiredPlaybackSeconds + 30) * 2));
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const result = await command("Runtime.evaluate", {
      returnByValue: true,
      expression: `(() => {
        const media = document.getElementById('tawny-player-media');
        if (!media) return { exists: false, title: document.title };
        return {
          exists: true,
          hasTransport: Boolean(window.TawnyTransport),
          hasShaka: Boolean(window.shaka),
          attachEval: window.__tawnyAttachEval || null,
          transportDebug: window.__tawnyTransportDebug || null,
          readyState: media.readyState,
          networkState: media.networkState,
          currentTime: media.currentTime,
          duration: Number.isFinite(media.duration) ? media.duration : null,
          paused: media.paused,
          error: media.error ? { code: media.error.code, message: media.error.message } : null,
          src: media.currentSrc,
          buffered: Array.from({ length: media.buffered.length }, (_, index) => [
            media.buffered.start(index), media.buffered.end(index)
          ]),
          transport: document.querySelector('.resolver-pill')?.textContent?.trim() || null
        };
      })()`,
    });
    state = result.result.value;
    if (state.exists) {
      const previous = samples.at(-1);
      samples.push({
        elapsedSeconds: attempt / 2,
        currentTime: state.currentTime,
        readyState: state.readyState,
        networkState: state.networkState,
        buffered: state.buffered,
        transportDebug: state.transportDebug,
        rewind: Boolean(
          previous &&
            previous.readyState >= 2 &&
            state.readyState >= 2 &&
            state.currentTime + 2 < previous.currentTime,
        ),
      });
    }
    if (!faultInjected && faultAtSeconds >= 0 && state.currentTime >= faultAtSeconds) {
      await command("Runtime.evaluate", {
        expression: `document.getElementById('tawny-player-media')?.dispatchEvent(new Event('error'))`,
      });
      faultInjected = true;
    }
    if (
      state.exists &&
      state.readyState >= 2 &&
      state.duration > 0 &&
      state.buffered.length > 0 &&
      state.currentTime >= requiredPlaybackSeconds
    ) {
      break;
    }
    await delay(500);
  }

  console.log(
    JSON.stringify({ state, faultInjected, samples, networkFailures, consoleMessages }, null, 2),
  );
  await command("Browser.close").catch(() => {});
  socket.close();
  if (
    !state ||
    state.readyState < 2 ||
    !state.duration ||
    !state.buffered.length ||
    state.currentTime < requiredPlaybackSeconds ||
    samples.some((sample) => sample.rewind) ||
    (faultAtSeconds >= 0 && !faultInjected)
  ) {
    process.exitCode = 1;
  }
}

inspect()
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  })
  .finally(async () => {
    await delay(500);
    if (!browser.killed) browser.kill();
  });
