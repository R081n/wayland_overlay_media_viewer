use std::time::Duration;

use wayland_overlay_media_viewer::{
    position::PopupPosition,
    spawner::{
        ObjectCloseCondition, ObjectMessage, ObjectType, PopupImage, PopupInAnimation,
        PopupInteraction, PopupOutAnimation,
    },
};

fn main() {
    let sender = wayland_overlay_media_viewer::startup();

    for line in std::io::stdin().lines() {
        let line = line.unwrap();

        if let Ok(dir) = std::fs::read_dir(&line) {
            let sender = sender.clone();
            std::thread::spawn(move || {
                for entry in dir.flatten() {
                    sender.send(ObjectMessage {
                        kind: ObjectType::Image(PopupImage {
                            uri: entry.path().to_string_lossy().into_owned(),
                        }),
                        position: PopupPosition::Random,
                        popup_animation: PopupInAnimation::SlideFadeIn,
                        behaviour: PopupInteraction::ClickThough,
                        opacity: 0.4,
                        close_animation: PopupOutAnimation::FadeOut,
                        close_condition: ObjectCloseCondition {
                            duration: Some(Duration::from_secs(2)),
                            click: None,
                        },
                    });
                    std::thread::sleep(Duration::from_millis(500));
                }
            });
        }

        sender.send(ObjectMessage {
            kind: ObjectType::Image(PopupImage { uri: line }),
            position: PopupPosition::Random,
            popup_animation: PopupInAnimation::SlideFadeIn,
            behaviour: PopupInteraction::ClickThough,
            opacity: 0.4,
            close_animation: PopupOutAnimation::FadeOut,
            close_condition: ObjectCloseCondition {
                duration: Some(Duration::from_secs(2)),
                click: None,
            },
        });
    }
}
