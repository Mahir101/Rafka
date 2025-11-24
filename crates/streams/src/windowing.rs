use std::time::Duration;

/// Window types for stream processing
#[derive(Debug, Clone)]
pub enum WindowType {
    Tumbling,
    Hopping,
    Sliding,
    Session,
}

/// Time window configuration
#[derive(Debug, Clone)]
pub struct TimeWindow {
    pub window_type: WindowType,
    pub size: Duration,
    pub advance: Option<Duration>, // For hopping windows
}

impl TimeWindow {
    pub fn tumbling(size: Duration) -> Self {
        Self {
            window_type: WindowType::Tumbling,
            size,
            advance: None,
        }
    }

    pub fn hopping(size: Duration, advance: Duration) -> Self {
        Self {
            window_type: WindowType::Hopping,
            size,
            advance: Some(advance),
        }
    }

    pub fn sliding(size: Duration) -> Self {
        Self {
            window_type: WindowType::Sliding,
            size,
            advance: None,
        }
    }
}

/// Session window configuration
#[derive(Debug, Clone)]
pub struct SessionWindow {
    pub inactivity_gap: Duration,
}

impl SessionWindow {
    pub fn with_gap(gap: Duration) -> Self {
        Self {
            inactivity_gap: gap,
        }
    }
}

/// Generic window trait
pub trait Window: Send + Sync {
    fn window_type(&self) -> WindowType;
}

impl Window for TimeWindow {
    fn window_type(&self) -> WindowType {
        self.window_type.clone()
    }
}

impl Window for SessionWindow {
    fn window_type(&self) -> WindowType {
        WindowType::Session
    }
}
