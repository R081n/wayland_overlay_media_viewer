use std::time::Duration;

use rand::seq::SliceRandom;
use wayland_overlay_media_viewer::{
    position::PopupPosition,
    spawner::{
        ObjectCloseCondition, ObjectMessage, ObjectType, PopupImage, PopupInAnimation,
        PopupInteraction, PopupOutAnimation, PopupVideo,
    },
};

fn main() {
    let sender = wayland_overlay_media_viewer::startup();

    for line in std::io::stdin().lines() {
        let line = line.unwrap();

        if let Ok(dir) = std::fs::read_dir(&line) {
            let sender = sender.clone();
            std::thread::spawn(move || {
                let mut list: Vec<_> = dir.flatten().collect();
                list.shuffle(&mut rand::rng());

                for entry in list {
                    sender.send(ObjectMessage {
                        kind: ObjectType::Image(PopupImage {
                            uri: entry.path().to_string_lossy().into_owned(),
                        }),

                        position: PopupPosition::Random,
                        popup_animation: PopupInAnimation::SlideFadeIn,
                        behaviour: PopupInteraction::ClickThough,
                        opacity: 0.9,
                        close_animation: PopupOutAnimation::FadeOut,
                        close_condition: ObjectCloseCondition {
                            duration: Some(Duration::from_secs_f64(20.0)),
                            click: None,
                        },
                    });
                    std::thread::sleep(Duration::from_millis(1000));
                }
            });
        }

        sender.send(ObjectMessage {
            kind: to_type(line),
            position: PopupPosition::Random,
            popup_animation: PopupInAnimation::SlideFadeIn,
            behaviour: PopupInteraction::ClickThough,
            opacity: 0.9,
            close_animation: PopupOutAnimation::FadeOut,
            close_condition: ObjectCloseCondition {
                duration: Some(Duration::from_secs(1000)),
                click: None,
            },
        });
    }
}

fn to_type(string: String) -> ObjectType {
    if string.ends_with("mp4") || string.ends_with("webm") {
        ObjectType::Video(PopupVideo { uri: string })
    } else {
        ObjectType::Image(PopupImage { uri: string })
    }
}
