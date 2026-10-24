//! Pages of the open panel.
pub mod chat;
pub mod focus;
pub mod music;
pub mod shelf;
pub mod today;
pub mod todo;
pub mod water;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageId {
    Today,
    Music,
    Focus,
    Todo,
    Water,
    Shelf,
    Chat,
}

impl PageId {
    pub fn as_str(&self) -> &'static str {
        match self {
            PageId::Today => "today",
            PageId::Music => "music",
            PageId::Focus => "focus",
            PageId::Todo => "todo",
            PageId::Water => "water",
            PageId::Shelf => "shelf",
            PageId::Chat => "chat",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "music" => PageId::Music,
            "focus" => PageId::Focus,
            "todo" => PageId::Todo,
            "water" => PageId::Water,
            "shelf" => PageId::Shelf,
            "chat" => PageId::Chat,
            _ => PageId::Today,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            PageId::Today => "Hari Ini",
            PageId::Music => "Musik",
            PageId::Focus => "Fokus",
            PageId::Todo => "Tugas",
            PageId::Water => "Air",
            PageId::Shelf => "Shelf",
            PageId::Chat => "Chat",
        }
    }
}
