use eframe::{egui};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use dfbhd_mus::mm;

// Represents an individual button item.
struct ButtonItem {
    label: String,
    processing: bool,
}

struct MyApp {
    items: Vec<ButtonItem>,
    auto_run: bool,
    drag_index: Option<usize>,
    tx: mpsc::Sender<usize>,
    rx: mpsc::Receiver<usize>,
}

impl Default for MyApp {
    fn default() -> Self {
        // Create a channel for background processing notifications.
        let (tx, rx) = mpsc::channel();
        Self {
            items: vec![
                ButtonItem { label: "Button 1".to_owned(), processing: false },
                ButtonItem { label: "Button 2".to_owned(), processing: false },
                ButtonItem { label: "Button 3".to_owned(), processing: false },
            ],
            auto_run: false,
            drag_index: None,
            tx,
            rx,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process finished background tasks.
        while let Ok(finished_index) = self.rx.try_recv() {
            if finished_index < self.items.len() {
                self.items[finished_index].processing = false;
            }
            // Auto-run: if enabled, start processing on the next button.
            if self.auto_run {
                let next_index = finished_index + 1;
                if next_index < self.items.len() {
                    self.start_processing(next_index);
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Extra button above the list.
            if ui.button("Extra Button").clicked() {
                // Replace this with your own action.
                println!("Extra button clicked!");
            }
            ui.add_space(10.0);
            ui.checkbox(&mut self.auto_run, "Auto-run next");
            ui.separator();

            // Render the list of buttons.
            for i in 0..self.items.len() {
                let item = mm(&self.items[i]);
                let button_label = if item.processing {
                    format!("{} (processing)", item.label)
                } else {
                    item.label.clone()
                };

                let response = ui.add(egui::Button::new(button_label));

                // Start tracking drag if the user starts dragging.
                if response.drag_started() {
                    self.drag_index = Some(i);
                }

                // Swap items when dragging over another button.
                if response.dragged() && response.hovered() {
                    if let Some(src) = self.drag_index {
                        if src != i {
                            mm(&self.items).swap(src, i);
                            self.drag_index = Some(i);
                        }
                    }
                }

                // Start background processing on click if not already processing.
                if response.clicked() && !item.processing {
                    self.start_processing(i);
                }
            }
        });
        // Request a repaint so the UI stays responsive.
        ctx.request_repaint();
    }
}

impl MyApp {
    fn start_processing(&mut self, index: usize) {
        if index >= self.items.len() {
            return;
        }
        // Mark the button as processing.
        self.items[index].processing = true;

        // Clone the sender to move into the background thread.
        let tx = self.tx.clone();
        thread::spawn(move || {
            // Simulate background work.
            thread::sleep(Duration::from_secs(3));
            // Signal that processing for the given index is complete.
            tx.send(index).unwrap();
        });
    }
}

fn main() {
    let app = MyApp::default();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "t800",
        native_options,
        Box::new(move |_| Ok(Box::new(app)))
    ).unwrap();
}
