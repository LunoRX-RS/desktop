//! The Dock icon follows the macOS appearance setting (Light, Dark or Auto). The bundle's `.icns` is one static
//! picture, so at runtime the app hands AppKit the matching variant. Finder, Launchpad and a pinned tile while the
//! app is not running still read the bundle, so they keep the light art; the running app's Dock tile follows.
//!
//! macOS posts `AppleInterfaceThemeChangedNotification` on the distributed notification centre whenever the effective
//! appearance changes — a manual switch or the scheduled Auto change — whether or not the app's window is visible. A
//! per-window theme event would miss it while the app is hidden, so the observer is registered app-wide.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSImage};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSData, NSDistributedNotificationCenter, NSNotification,
    NSOperationQueue, NSString,
};

/// The light tile with the dark glyph: the picture the bundle was built with.
const LIGHT: &[u8] = include_bytes!("../../icons/icon.png");
/// The dark tile with the light glyph, rendered from the light master by
/// `desktop/scripts/gen-macos-dark-icon.swift`.
const DARK: &[u8] = include_bytes!("../../icons/dark/icon-512.png");

/// The notification macOS posts when the effective appearance changes.
const THEME_CHANGED: &str = "AppleInterfaceThemeChangedNotification";

/// Pure: the picture for a name answered by AppKit's best match. Anything that cannot be read as one of the two
/// keeps the icon that is already up.
pub fn variant_for(matched: Option<&NSString>) -> Option<&'static [u8]> {
    let matched = matched?;
    if matched == unsafe { NSAppearanceNameDarkAqua } {
        Some(DARK)
    } else if matched == unsafe { NSAppearanceNameAqua } {
        Some(LIGHT)
    } else {
        None
    }
}

/// Which of the two pictures the effective appearance renders as. Main thread only.
fn best_match(mtm: MainThreadMarker) -> Option<Retained<NSString>> {
    let names = NSArray::from_slice(&[unsafe { NSAppearanceNameAqua }, unsafe {
        NSAppearanceNameDarkAqua
    }]);
    NSApplication::sharedApplication(mtm)
        .effectiveAppearance()
        .bestMatchFromAppearancesWithNames(&names)
}

/// Sets the Dock icon to the variant the current appearance asks for.
fn apply(mtm: MainThreadMarker) {
    let Some(bytes) = variant_for(best_match(mtm).as_deref()) else {
        return;
    };
    let data = NSData::with_bytes(bytes);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        // SAFETY: plain AppKit call on the main thread with a valid image.
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

/// Applies the current appearance now, then keeps the Dock icon in step with every later appearance change.
/// Call from `setup`, on the main thread.
pub fn install() {
    if let Some(mtm) = MainThreadMarker::new() {
        apply(mtm);
    }
    observe_theme();
}

/// Observes the system-wide theme notification and re-applies the icon on the main queue when it fires. The
/// observer token is intentionally leaked so the registration lives for the whole process.
fn observe_theme() {
    let center = NSDistributedNotificationCenter::defaultCenter();
    let name = NSString::from_str(THEME_CHANGED);
    let queue = NSOperationQueue::mainQueue();
    let block = RcBlock::new(|_note: NonNull<NSNotification>| {
        if let Some(mtm) = MainThreadMarker::new() {
            apply(mtm);
        }
    });
    // SAFETY: the block only touches AppKit on the main queue and captures nothing; the returned observer is kept
    // alive for the process lifetime so the registration is never dangling.
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(&name), None, Some(&queue), &block)
    };
    std::mem::forget(observer);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_appearance_picks_its_own_picture() {
        let dark = NSString::from_str("NSAppearanceNameDarkAqua");
        let aqua = NSString::from_str("NSAppearanceNameAqua");
        assert_eq!(variant_for(Some(&dark)), Some(DARK));
        assert_eq!(variant_for(Some(&aqua)), Some(LIGHT));
    }

    #[test]
    fn an_unreadable_appearance_changes_nothing() {
        assert_eq!(variant_for(None), None);
        assert_eq!(variant_for(Some(&NSString::from_str("whatever"))), None);
    }
}
