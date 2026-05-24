use crate::markdown::{RenderedMarkdown, render_markdown};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;

pub type SharedSessions = Arc<RwLock<HashMap<Uuid, Session>>>;

#[derive(Debug, Clone, Deserialize)]
pub struct SessionInput {
    pub path: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateEvent {
    pub revision: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScrollInput {
    pub line: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScrollEvent {
    pub line: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloseEvent {
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    Update(UpdateEvent),
    Scroll(ScrollEvent),
    Close(CloseEvent),
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub path: String,
    pub title: String,
    pub content: String,
    pub rendered: RenderedMarkdown,
    pub revision: u64,
    pub updates: broadcast::Sender<SessionEvent>,
}

impl Session {
    pub fn new(input: SessionInput) -> Self {
        let (updates, _) = broadcast::channel(64);
        let id = Uuid::new_v4();
        let rendered = render_markdown(&input.content, Some(id));
        Self {
            id,
            path: input.path,
            title: input.title,
            rendered,
            content: input.content,
            revision: 1,
            updates,
        }
    }

    pub fn update(&mut self, input: SessionInput) -> UpdateEvent {
        self.path = input.path;
        self.title = input.title;
        self.rendered = render_markdown(&input.content, Some(self.id));
        self.content = input.content;
        self.revision += 1;

        let event = UpdateEvent {
            revision: self.revision,
        };
        let _ = self.updates.send(SessionEvent::Update(event.clone()));
        event
    }

    pub fn scroll(&self, input: ScrollInput) -> ScrollEvent {
        let event = ScrollEvent { line: input.line };
        let _ = self.updates.send(SessionEvent::Scroll(event.clone()));
        event
    }

    pub fn close(&self) -> CloseEvent {
        let event = CloseEvent { reason: "stopped" };
        let _ = self.updates.send(SessionEvent::Close(event.clone()));
        event
    }
}

pub fn new_store() -> SharedSessions {
    Arc::new(RwLock::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_increments_revision() {
        let mut session = Session::new(SessionInput {
            path: "/tmp/a.md".to_string(),
            title: "a.md".to_string(),
            content: "# A".to_string(),
        });
        let mut receiver = session.updates.subscribe();

        let event = session.update(SessionInput {
            path: "/tmp/a.md".to_string(),
            title: "a.md".to_string(),
            content: "# B".to_string(),
        });

        assert_eq!(event.revision, 2);
        assert_eq!(session.revision, 2);
        assert!(session.rendered.html.contains("<h1>B</h1>"));
        match receiver.try_recv().unwrap() {
            SessionEvent::Update(update) => assert_eq!(update.revision, 2),
            SessionEvent::Scroll(_) | SessionEvent::Close(_) => panic!("expected update event"),
        }
    }

    #[test]
    fn scroll_sends_scroll_event() {
        let session = Session::new(SessionInput {
            path: "/tmp/a.md".to_string(),
            title: "a.md".to_string(),
            content: "# A".to_string(),
        });
        let mut receiver = session.updates.subscribe();

        let event = session.scroll(ScrollInput { line: 12 });

        assert_eq!(event.line, 12);
        match receiver.try_recv().unwrap() {
            SessionEvent::Scroll(scroll) => assert_eq!(scroll.line, 12),
            SessionEvent::Update(_) | SessionEvent::Close(_) => panic!("expected scroll event"),
        }
    }

    #[test]
    fn close_sends_close_event() {
        let session = Session::new(SessionInput {
            path: "/tmp/a.md".to_string(),
            title: "a.md".to_string(),
            content: "# A".to_string(),
        });
        let mut receiver = session.updates.subscribe();

        let event = session.close();

        assert_eq!(event.reason, "stopped");
        match receiver.try_recv().unwrap() {
            SessionEvent::Close(close) => assert_eq!(close.reason, "stopped"),
            SessionEvent::Update(_) | SessionEvent::Scroll(_) => panic!("expected close event"),
        }
    }
}
