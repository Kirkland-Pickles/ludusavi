use std::sync::mpsc::{Receiver, Sender, channel};

use super::background::BackupNotice;

pub struct DesktopNotifications {
    sender: Sender<BackupNotice>,
    errors: Receiver<String>,
}

impl DesktopNotifications {
    pub fn new() -> Result<Self, String> {
        let (sender, notices) = channel::<BackupNotice>();
        let (errors, receiver) = channel();
        std::thread::Builder::new()
            .name("backup-notifications".into())
            .spawn(move || {
                let mut last_error = None;
                let mut report = |why: String| {
                    log::warn!("{why}");
                    if last_error.as_ref() != Some(&why) {
                        let _ = errors.send(why.clone());
                        last_error = Some(why);
                    }
                };
                #[cfg(target_os = "macos")]
                match mac_usernotifications::blocking::request_auth() {
                    Ok(true) => {}
                    Ok(false) => {
                        report("Allow Ludusavi notifications in System Settings to see backup notifications.".into())
                    }
                    Err(why) => report(format!(
                        "Unable to enable backup notifications: {why}. Run the installed Ludusavi.app."
                    )),
                }
                // Keep start and result notifications in order, without blocking the GUI.
                for notice in notices {
                    match send(&notice) {
                        Ok(()) => log::info!("Desktop backup notification submitted: {}", notice.title),
                        Err(why) => report(format!("Unable to show backup notifications: {why}")),
                    }
                }
            })
            .map_err(|why| why.to_string())?;
        Ok(Self {
            sender,
            errors: receiver,
        })
    }

    pub fn notify(&self, notice: &BackupNotice) -> Result<(), String> {
        self.sender.send(notice.clone()).map_err(|why| why.to_string())
    }

    pub fn errors(&self) -> impl Iterator<Item = String> + '_ {
        self.errors.try_iter()
    }
}

#[cfg(target_os = "linux")]
fn send(notice: &BackupNotice) -> Result<(), String> {
    notify_rust::Notification::new()
        .appname("Ludusavi")
        .summary(notice.title)
        .body(&notice.body)
        .icon(crate::prelude::LINUX_APP_ID)
        .hint(notify_rust::Hint::DesktopEntry(crate::prelude::LINUX_APP_ID.into()))
        .urgency(if notice.failed {
            notify_rust::Urgency::Critical
        } else {
            notify_rust::Urgency::Normal
        })
        .show()
        .map(|_| ())
        .map_err(|why| why.to_string())
}

#[cfg(target_os = "macos")]
fn send(notice: &BackupNotice) -> Result<(), String> {
    mac_usernotifications::Notification::new()
        .title(notice.title)
        .message(&notice.body)
        .send_blocking()
        .map(|_| ())
        .map_err(|why| why.to_string())
}
