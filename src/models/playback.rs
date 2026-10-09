use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackProtocol {
    Sabr,
    Hls,
    Dash,
    Progressive,
    EmbedFallback,
}

/// An inclusive byte range within an adaptive media resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackByteRange {
    pub start: u64,
    pub end: u64,
}

/// A single audio or video representation used by an adaptive transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackTrack {
    pub kind: PlaybackTrackKind,
    pub url: String,
    pub mime_type: String,
    #[serde(default)]
    pub bitrate: Option<u64>,
    #[serde(default)]
    pub content_length: Option<u64>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<u32>,
    #[serde(default)]
    pub quality_label: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub init_range: Option<PlaybackByteRange>,
    #[serde(default)]
    pub index_range: Option<PlaybackByteRange>,
    #[serde(default)]
    pub request_headers: Vec<PlaybackRequestHeader>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackTrackKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackSource {
    pub protocol: PlaybackProtocol,
    pub url: String,
    pub mime_type: Option<String>,
    pub po_token: Option<String>,
    pub expires_at: Option<String>,
    #[serde(default)]
    pub quality_label: Option<String>,
    #[serde(default)]
    pub request_headers: Vec<PlaybackRequestHeader>,
    /// Adaptive representations. An empty list keeps the legacy single-URL
    /// progressive/manifest resolver contract backwards compatible.
    #[serde(default)]
    pub tracks: Vec<PlaybackTrack>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackRequestHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackSession {
    pub primary: PlaybackSource,
    #[serde(default)]
    pub alternatives: Vec<PlaybackSource>,
    pub fallback_url: String,
}

/// Whose side a playback failure is on, which decides what can be done about
/// it: nothing (YouTube's own rules), a fix to the deployment, or a fix to
/// Tawny's code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackFailureOrigin {
    /// YouTube refused the video: region, privacy, age, removal, or YouTube
    /// blocking the server itself. Tawny is working as intended.
    Youtube,
    /// Something Tawny runs on is down, misconfigured or out of date - a
    /// sidecar, an environment variable, the yt-dlp pin. Whoever runs the
    /// server can fix it.
    Setup,
    /// Tawny's own code got it wrong.
    Bug,
    /// An error Tawny does not recognise, so it cannot say whose it is.
    Unknown,
}

impl PlaybackFailureOrigin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Youtube => "Blocked by YouTube",
            Self::Setup => "Server setup problem",
            Self::Bug => "Tawny bug",
            Self::Unknown => "Unrecognised error",
        }
    }
}

/// Why a video cannot be played, sent to the player instead of a bare string
/// so it can say whose problem it is and what, if anything, fixes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackFailure {
    pub origin: PlaybackFailureOrigin,
    /// What went wrong, as one sentence.
    pub summary: String,
    /// What the viewer or whoever runs the server can do, when anything helps.
    #[serde(default)]
    pub remedy: Option<String>,
    /// The underlying error verbatim, for bug reports and the server log.
    #[serde(default)]
    pub detail: Option<String>,
    /// Whether asking again shortly is likely to succeed: a busy extractor, a
    /// rate limit or a timeout, as opposed to a video YouTube will never serve.
    /// The player retries these by itself before showing the failure.
    #[serde(default)]
    pub retryable: bool,
}

impl PlaybackFailure {
    pub fn new(origin: PlaybackFailureOrigin, summary: impl Into<String>) -> Self {
        Self {
            origin,
            summary: summary.into(),
            remedy: None,
            detail: None,
            retryable: false,
        }
    }

    pub fn remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        self.detail = (!detail.trim().is_empty()).then_some(detail);
        self
    }
}

impl std::fmt::Display for PlaybackFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.origin.label(), self.summary)?;
        if let Some(detail) = &self.detail {
            write!(f, " ({detail})")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The viewer
//
// What one account keeps, read by nearly every screen. The catalog (every
// channel and video the instance has met) stays on the server; each screen
// asks for the videos it shows. See docs/architecture.md.
// ---------------------------------------------------------------------------
