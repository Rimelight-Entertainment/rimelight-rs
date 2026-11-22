use std::cell::Cell;
use std::rc::Rc;

use spike_app::AppWindow;
use slint::ComponentHandle;
use spike_kit::ToastQueue;

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let queue = ToastQueue::new();

    #[cfg(not(feature = "manual-trait"))]
    queue.attach(spike_app::WindowToastSink::new(&ui));
    #[cfg(feature = "manual-trait")]
    queue.attach(spike_app::manual::ManualSink::new(&ui));

    let counter = Rc::new(Cell::new(0u32));
    ui.on_push_toast({
        let queue = queue.clone();
        move || {
            let n = counter.get() + 1;
            counter.set(n);
            // Every second toast is tone 1 (accent border); the rest are neutral. Each lasts 4 s.
            queue.push(format!("Toast {n}"), (n % 2) as i32, 4000);
        }
    });

    ui.run()
}
