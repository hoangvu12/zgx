//! Whether the Wayland compositor blurs behind a window that asks
//! (`WindowCapabilities::blur_behind`): whether it has a protocol winit's
//! `set_blur` asks through, and whether that protocol blurs now. Read once,
//! on a connection of its own, so an app on a winit from crates.io (which
//! has no native popups, and no client of zgx's on its display) is told too.
//! KWin has `org_kde_kwin_blur_manager` while its blur effect is on, as a
//! launch finds it; Hyprland and others `ext-background-effect-v1`,
//! which only the repository's winit asks through, and through it alone
//! where a compositor has it. That protocol's manager says, once bound,
//! whether the compositor can blur now (its `capabilities`): without `blur`
//! it applies none, whatever region a window asks for. GNOME, sway and
//! Weston have neither, and glass there shows what is behind it sharp.
use std::sync::OnceLock;
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    protocol::wl_registry::{self, WlRegistry},
};
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_manager_v1::{
    self, Capability, ExtBackgroundEffectManagerV1,
};

const KWIN_BLUR: &str = "org_kde_kwin_blur_manager";
const BACKGROUND_EFFECT: &str = "ext_background_effect_manager_v1";

/// What the registry announced (each global's name and interface), and
/// the background-effect manager's `capabilities` once it is bound and has
/// sent them.
#[derive(Default)]
struct Registry {
    globals: Vec<(u32, String)>,
    effects: Option<u32>,
}
impl Dispatch<WlRegistry, ()> for Registry {
    fn event(
        registry: &mut Self,
        _: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            registry.globals.push((name, interface));
        }
    }
}
impl Dispatch<ExtBackgroundEffectManagerV1, ()> for Registry {
    fn event(
        registry: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        event: ext_background_effect_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_background_effect_manager_v1::Event::Capabilities { flags } = event {
            registry.effects = Some(match flags {
                WEnum::Value(flags) => flags.bits(),
                WEnum::Unknown(bits) => bits,
            });
        }
    }
}

/// Whether a compositor with `globals` blurs behind a window whose winit
/// (with `patched`, the repository's) asks it to: `effects` is what the
/// background-effect manager said it can apply, where it has one. The
/// repository's winit asks through that manager alone where there is one,
/// so it blurs only while the manager says `blur` (a manager that said
/// nothing has said no); otherwise, and always upstream, through KWin's.
fn blurs(globals: &[&str], patched: bool, effects: Option<u32>) -> bool {
    if patched && globals.contains(&BACKGROUND_EFFECT) {
        return effects.is_some_and(|flags| flags & Capability::Blur.bits() != 0);
    }
    globals.contains(&KWIN_BLUR)
}

/// Whether this session's Wayland compositor blurs behind a window that asks.
pub(crate) fn offered() -> bool {
    static OFFERED: OnceLock<bool> = OnceLock::new();
    *OFFERED.get_or_init(|| {
        let patched = cfg!(zgx_winit_patched);
        let read = || -> Option<bool> {
            let connection = Connection::connect_to_env().ok()?;
            let mut queue = connection.new_event_queue();
            let handle = queue.handle();
            let wl_registry = connection.display().get_registry(&handle, ());
            let mut registry = Registry::default();
            queue.roundtrip(&mut registry).ok()?;
            // The manager's capabilities come as it is bound (only the
            // repository's winit asks through it).
            let manager = (registry.globals.iter())
                .find(|(_, interface)| interface == BACKGROUND_EFFECT)
                .map(|(name, _)| *name);
            if patched && let Some(name) = manager {
                let _: ExtBackgroundEffectManagerV1 = wl_registry.bind(name, 1, &handle, ());
                queue.roundtrip(&mut registry).ok()?;
            }
            let globals = (registry.globals.iter())
                .map(|(_, interface)| interface.as_str())
                .collect::<Vec<_>>();
            Some(blurs(&globals, patched, registry.effects))
        };
        read().unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::{BACKGROUND_EFFECT, KWIN_BLUR, blurs};
    use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_manager_v1::Capability;

    #[test]
    fn a_compositor_blurs_if_it_has_a_protocol_winit_asks_through() {
        let blur = Some(Capability::Blur.bits());
        let kwin = ["wl_compositor", KWIN_BLUR];
        let hyprland = ["wl_compositor", BACKGROUND_EFFECT];
        let gnome = ["wl_compositor", "xdg_wm_base"];
        assert!(blurs(&kwin, false, None) && blurs(&kwin, true, None));
        assert!(blurs(&hyprland, true, blur));
        // Upstream winit asks through KWin's alone.
        assert!(!blurs(&hyprland, false, blur));
        assert!(!blurs(&gnome, true, None) && !blurs(&gnome, false, None));
    }

    /// `ext-background-effect-v1` blurs only while its manager says it can:
    /// one with blur off says no capabilities, and a region asked for stays
    /// sharp. A compositor with both protocols is asked through that one
    /// alone by the repository's winit, so KWin's doesn't make up for it.
    #[test]
    fn a_background_effect_manager_blurs_only_while_it_says_it_can() {
        let hyprland = ["wl_compositor", BACKGROUND_EFFECT];
        let kwin = ["wl_compositor", KWIN_BLUR, BACKGROUND_EFFECT];
        assert!(!blurs(&hyprland, true, Some(0)));
        // A manager that said nothing has said no.
        assert!(!blurs(&hyprland, true, None));
        // A capability this zgx doesn't know of, beside blur.
        assert!(blurs(&hyprland, true, Some(Capability::Blur.bits() | 0b10)));
        assert!(!blurs(&hyprland, true, Some(0b10)));
        assert!(!blurs(&kwin, true, Some(0)));
        assert!(blurs(&kwin, true, Some(Capability::Blur.bits())));
        // Upstream winit asks through KWin's, whatever the other says.
        assert!(blurs(&kwin, false, Some(0)));
    }
}
