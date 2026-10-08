use std::time::Instant;

use iced::{alignment, padding};

use crate::gui::{
    common::Message,
    style,
    widget::{Button, Container, Row, text},
};

pub struct Notification {
    text: String,
    created: Instant,
    expires: Option<u64>,
    action: Option<(String, Message)>,
}

impl Notification {
    pub fn new(text: String) -> Self {
        Self {
            text,
            created: Instant::now(),
            expires: None,
            action: None,
        }
    }

    pub fn expires(mut self, expires: u64) -> Self {
        self.expires = Some(expires);
        self
    }

    pub fn action(mut self, label: &str, message: Message) -> Self {
        self.action = Some((label.into(), message));
        self
    }

    pub fn expired(&self) -> bool {
        match self.expires {
            None => false,
            Some(expires) => (Instant::now() - self.created).as_secs() > expires,
        }
    }

    pub fn view(&self) -> Container {
        let mut content = Row::new()
            .spacing(10)
            .align_y(iced::Alignment::Center)
            .push(text(self.text.clone()));
        if let Some((label, message)) = &self.action {
            content = content.push(
                Button::new(text(label.clone()))
                    .padding(3)
                    .class(style::Button::Primary)
                    .on_press(message.clone()),
            );
        }
        Container::new(
            Container::new(content)
                .padding([3, 40])
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .class(style::Container::Notification),
        )
        .padding(padding::bottom(5))
    }
}
