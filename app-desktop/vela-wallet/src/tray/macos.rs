//! The menu-bar extra on macOS: an `NSStatusItem` with the Vela mark as a
//! template image, the total as its title, and a menu.
//!
//! Plain `msg_send!` against AppKit's classes (the objc2 generation gpui
//! pins), so no new binding crate or feature enters the build. Main thread
//! only — gpui's, where every caller runs.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{ClassType, DeclaredClass, class, declare_class, msg_send, msg_send_id, mutability, sel};
use objc2_foundation::{MainThreadMarker, NSData, NSSize, NSString};

use super::{TrayCommand, TrayModel, press};

declare_class!(
    /// The menu items' target: each action is one [`TrayCommand`].
    struct Target;

    unsafe impl ClassType for Target {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "VelaMenuBarTarget";
    }

    impl DeclaredClass for Target {}

    unsafe impl Target {
        #[method(velaOpen:)]
        fn open(&self, _sender: *mut AnyObject) {
            press(TrayCommand::Open);
        }

        #[method(velaSend:)]
        fn send(&self, _sender: *mut AnyObject) {
            press(TrayCommand::Send);
        }

        #[method(velaReceive:)]
        fn receive(&self, _sender: *mut AnyObject) {
            press(TrayCommand::Receive);
        }
    }
);

impl Target {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(());
        unsafe { msg_send_id![super(this), init] }
    }
}

struct Extra {
    item: Retained<AnyObject>,
    target: Retained<Target>,
}

thread_local! {
    static EXTRA: RefCell<Option<Extra>> = const { RefCell::new(None) };
}

/// `NSVariableStatusItemLength`.
const VARIABLE_LENGTH: f64 = -1.;
/// `NSImageLeading`: the mark before the figure.
const IMAGE_LEADING: isize = 7;

fn ns(text: &str) -> Retained<NSString> {
    NSString::from_str(text)
}

/// The extra, made the first time.
fn extra(mtm: MainThreadMarker, mark: &[u8]) -> Option<(Retained<AnyObject>, Retained<Target>)> {
    EXTRA.with(|slot| {
        if let Some(extra) = slot.borrow().as_ref() {
            return Some((extra.item.clone(), extra.target.clone()));
        }
        // SAFETY: AppKit's documented class methods, on the main thread
        // (`mtm`); every object is retained by `Retained` or by AppKit.
        unsafe {
            let bar: Retained<AnyObject> = msg_send_id![class!(NSStatusBar), systemStatusBar];
            let item: Retained<AnyObject> =
                msg_send_id![&bar, statusItemWithLength: VARIABLE_LENGTH];
            let button: Option<Retained<AnyObject>> = msg_send_id![&item, button];
            if let Some(button) = button.as_ref() {
                let data = NSData::with_bytes(mark);
                let image: Option<Retained<AnyObject>> =
                    msg_send_id![msg_send_id![class!(NSImage), alloc], initWithData: &*data];
                if let Some(image) = image {
                    let _: () = msg_send![&image, setTemplate: true];
                    let _: () = msg_send![&image, setSize: NSSize::new(18., 18.)];
                    let _: () = msg_send![button, setImage: &*image];
                    let _: () = msg_send![button, setImagePosition: IMAGE_LEADING];
                }
            }
            let target = Target::new(mtm);
            *slot.borrow_mut() = Some(Extra {
                item: item.clone(),
                target: target.clone(),
            });
            Some((item, target))
        }
    })
}

fn menu_item(
    title: &str,
    action: Option<Sel>,
    target: Option<&Target>,
) -> Option<Retained<AnyObject>> {
    // SAFETY: NSMenuItem's designated initialiser; the target outlives the
    // menu (it is held in `EXTRA` for the extra's life).
    unsafe {
        let item: Option<Retained<AnyObject>> = msg_send_id![
            msg_send_id![class!(NSMenuItem), alloc],
            initWithTitle: &*ns(title),
            action: action,
            keyEquivalent: &*ns("")
        ];
        let item = item?;
        match target {
            Some(target) => {
                let _: () = msg_send![&item, setTarget: target];
            }
            None => {
                let _: () = msg_send![&item, setEnabled: false];
            }
        }
        Some(item)
    }
}

pub fn show(model: &TrayModel, mark: &[u8]) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some((item, target)) = extra(mtm, mark) else {
        return;
    };
    // SAFETY: as in `extra`.
    unsafe {
        let button: Option<Retained<AnyObject>> = msg_send_id![&item, button];
        if let Some(button) = button.as_ref() {
            // A space between the mark and the figure, as every extra sets it.
            let title = model
                .figure
                .as_deref()
                .map(|figure| format!(" {figure}"))
                .unwrap_or_default();
            let _: () = msg_send![button, setTitle: &*ns(&title)];
            let tip = [Some(model.open.as_str()), model.updated.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" — ");
            let _: () = msg_send![button, setToolTip: &*ns(&tip)];
        }
        let menu: Retained<AnyObject> =
            msg_send_id![msg_send_id![class!(NSMenu), alloc], initWithTitle: &*ns(&model.open)];
        // Disabled items are information, not controls: say so to AppKit.
        let _: () = msg_send![&menu, setAutoenablesItems: false];
        let add = |entry: Option<Retained<AnyObject>>| {
            if let Some(entry) = entry {
                let _: () = msg_send![&menu, addItem: &*entry];
            }
        };
        let separator = || -> Option<Retained<AnyObject>> {
            Some(msg_send_id![class!(NSMenuItem), separatorItem])
        };
        let mut said = false;
        if let Some(figure) = model.figure.as_deref() {
            add(menu_item(figure, None, None));
            said = true;
        }
        if let Some(updated) = model.updated.as_deref() {
            add(menu_item(updated, None, None));
            said = true;
        }
        if said {
            add(separator());
        }
        add(menu_item(&model.send, Some(sel!(velaSend:)), Some(&target)));
        add(menu_item(&model.receive, Some(sel!(velaReceive:)), Some(&target)));
        add(separator());
        add(menu_item(&model.open, Some(sel!(velaOpen:)), Some(&target)));
        let _: () = msg_send![&item, setMenu: &*menu];
    }
}

pub fn hide() {
    EXTRA.with(|slot| {
        if let Some(extra) = slot.borrow_mut().take() {
            // SAFETY: the item was made by this status bar.
            unsafe {
                let bar: Retained<AnyObject> = msg_send_id![class!(NSStatusBar), systemStatusBar];
                let _: () = msg_send![&bar, removeStatusItem: &*extra.item];
            }
        }
    });
}
