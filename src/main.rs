use wayland_overlay_media_viewer::spawner::{ObjectMessage, PopupImage};

fn main() {
    let sender = wayland_overlay_media_viewer::startup();

    for line in std::io::stdin().lines() {
        let line = line.unwrap();

        sender.send(ObjectMessage {
            kind: wayland_overlay_media_viewer::spawner::ObjectType::Image(PopupImage {
                uri: line,
            }),
            position: wayland_overlay_media_viewer::position::PopupPosition::Random,
            popup_animation: wayland_overlay_media_viewer::spawner::PopupAnimation::SlideFadeIn,
            behaviour: wayland_overlay_media_viewer::spawner::PopupInteraction::ClickThough,
            opacity: 0.4,
        });
    }
}
