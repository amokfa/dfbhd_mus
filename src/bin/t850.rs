use eframe::egui;
use eframe::egui::{Align2, Color32, FontId, Pos2, Rect, Response, Rounding, Sense, Ui, Vec2};
use std::sync::mpsc;
use std::thread;
use dfbhd_mus::mm;

// Data structure for each button
struct ButtonData {
    id: u32,              // Unique identifier
    label: String,        // Button text
    is_processing: bool,  // Whether the button is currently processing
}

// Main application struct
struct MyApp {
    buttons: Vec<ButtonData>,
    auto_start_next: bool,          // Toggle for auto-starting next button
    sender: mpsc::Sender<u32>,      // Channel sender for background tasks
    receiver: mpsc::Receiver<u32>,  // Channel receiver for task completion
}

impl MyApp {
    // Initialize the application
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        // Create 5 buttons with unique IDs and labels
        let buttons = (0..5)
            .map(|i| ButtonData {
                id: i,
                label: format!("Button {}", i),
                is_processing: false,
            })
            .collect();
        Self {
            buttons,
            auto_start_next: false,
            sender,
            receiver,
        }
    }

    // Update function called each frame
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Top button
            if ui.button("Top Button").clicked() {
                println!("Top Button clicked!"); // Placeholder action
            }

            // Checkbox to toggle auto-starting of next button
            ui.checkbox(&mut self.auto_start_next, "Auto start next");

            // Variables for drag and drop
            let mut button_rects = Vec::new();
            let mut dragged_index = None;
            let mut drag_released = false;

            // Vertical layout for the button list
            ui.vertical(|ui| {
                for (index, button) in self.buttons.iter().enumerate() {
                    let id = egui::Id::new(button.id);
                    let response = draggable_button(ui, &button.label, button.is_processing);
                    button_rects.push((index, response.rect));

                    // Handle button click to start processing
                    if response.clicked() {
                        mm(self).start_processing(index);
                    }

                    // Detect if this button is being dragged
                    if response.dragged() {
                        dragged_index = Some(index);
                    }

                    // Detect if drag has ended
                    if response.drag_released() {
                        drag_released = true;
                    }
                }
            });

            // Handle drag and drop reordering
            if let Some(index) = dragged_index {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let target_index = self.calculate_target_index(&button_rects, pointer_pos);
                    if drag_released {
                        self.reorder_button(index, target_index);
                    }
                }
            }
            // Check for completed background tasks
            while let Ok(finished_id) = self.receiver.try_recv() {
                if let Some(button) = self.buttons.iter_mut().find(|b| b.id == finished_id) {
                    button.is_processing = false;
                }
                // If auto-start is enabled, process the next button
                if self.auto_start_next {
                    if let Some(current_index) = self.buttons.iter().position(|b| b.id == finished_id) {
                        if let Some(next_index) = self.get_next_index(current_index) {
                            self.start_processing(next_index);
                        }
                    }
                }
            }
        });
    }

    // Start background processing for a button
    fn start_processing(&mut self, index: usize) {
        if !self.buttons[index].is_processing {
            self.buttons[index].is_processing = true;
            let sender = self.sender.clone();
            let id = self.buttons[index].id;
            thread::spawn(move || {
                // Simulate processing with a 2-second delay
                thread::sleep(std::time::Duration::from_secs(2));
                sender.send(id).unwrap();
            });
        }
    }

    // Calculate the target index for drag and drop based on pointer position
    fn calculate_target_index(&self, button_rects: &[(usize, Rect)], pointer_pos: Pos2) -> usize {
        let pointer_y = pointer_pos.y;
        for (i, &(_, rect)) in button_rects.iter().enumerate() {
            if pointer_y < rect.center().y {
                return i;
            }
        }
        button_rects.len()
    }

    // Reorder the buttons in the list
    fn reorder_button(&mut self, from_index: usize, to_index: usize) {
        if from_index != to_index {
            let button = self.buttons.remove(from_index);
            if to_index > from_index {
                self.buttons.insert(to_index - 1, button);
            } else {
                self.buttons.insert(to_index, button);
            }
        }
    }

    // Get the index of the next button
    fn get_next_index(&self, current_index: usize) -> Option<usize> {
        if current_index + 1 < self.buttons.len() {
            Some(current_index + 1)
        } else {
            None
        }
    }
}

// Custom function to create a draggable and clickable button
fn draggable_button(ui: &mut Ui, label: &str, is_processing: bool) -> Response {
    let desired_size = ui.spacing().interact_size.y * Vec2::new(10.0, 1.0);
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::click().union(Sense::drag()));
    if ui.is_rect_visible(rect) {
        let fill_color = if is_processing {
            Color32::YELLOW // Highlight when processing
        } else {
            ui.visuals().extreme_bg_color // Default color
        };
        ui.painter().rect_filled(rect, Rounding::same(2), fill_color);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::default(),
            ui.visuals().text_color(),
        );
    }
    response
}

// Implement the eframe::App trait
impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.update(ctx, frame);
    }
}

// Main function to run the application
fn main() {
    let app = MyApp::new();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native("My App", native_options, Box::new(|_cc| Ok(Box::new(app)))).unwrap();
}