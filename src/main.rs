use std::env;
use std::sync::{mpsc, Arc};

use cpal::platform::Host;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamError};

use iced::widget::canvas::{path, stroke::Stroke, Canvas, Cursor, Frame, Geometry, Program};
use iced::widget::{
    self, button, column, container, horizontal_rule, horizontal_space, image, row, scrollable,
    text, vertical_space,
};
use iced::{
    executor, keyboard, subscription, theme, Alignment, Application, Color, Command, Element,
    Event, Length, Point, Rectangle, Settings, Subscription, Theme,
};

use once_cell::sync::Lazy;

mod output_modal;

use output_modal::Modal;

static OUTPUT_SCROLLABLE_ID: Lazy<scrollable::Id> = Lazy::new(scrollable::Id::unique);

struct Waveform {
    rx: Arc<mpsc::Receiver<Vec<f32>>>,
}

impl<Message> Program<Message> for Waveform {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let data = self.rx.recv().unwrap();

        let mut frame = Frame::new(bounds.size());

        let mut path_builder = path::Builder::new();
        let slice_width = bounds.width / data.len() as f32;
        let mut x = 0.;

        for (i, v) in data.iter().enumerate() {
            let y = (v * bounds.height) / 2. + bounds.height / 2.;

            if i == 0 {
                path_builder.move_to(Point::new(x, y));
            } else {
                path_builder.line_to(Point::new(x, y));
            }

            x += slice_width;
        }

        let path = path_builder.build();
        frame.stroke(
            &path,
            Stroke::default().with_color(Color::BLACK).with_width(2.),
        );

        vec![frame.into_geometry()]
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    ShowOutputModal,
    HideOutputModal,
    Tick,
    SelectedDevice(String),
    Event(Event),
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum Page {
    Main,
    Waveform,
}

#[derive(Debug, Clone, Eq, PartialEq, Copy)]
#[allow(dead_code)]
enum Direction {
    Vertical,
    Horizontal,
    Multi,
}

#[allow(dead_code)]
pub struct ScrollableData {
    width: u16,
    margin: u16,
    scroller_width: u16,
    current_scroll_offset: scrollable::RelativeOffset,
}

#[allow(dead_code)]
struct App {
    theme: Theme,
    show_output_modal: bool,
    output_device_names: Vec<String>,
    output_scrollable: ScrollableData,
    page: Page,
    host: Host,
    output_stream: Option<Stream>,
    output_sender: mpsc::Sender<Vec<f32>>,
    output_reciever: Arc<mpsc::Receiver<Vec<f32>>>,
    background_image: Option<image::Handle>,
}

impl Default for App {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut bg_path = env::current_dir().unwrap();
        bg_path.push("bg.png");

        App {
            theme: Theme::Dark,
            show_output_modal: false,
            output_device_names: Vec::new(),
            output_scrollable: ScrollableData {
                width: 10,
                margin: 0,
                scroller_width: 10,
                current_scroll_offset: scrollable::RelativeOffset::START,
            },
            page: Page::Main,
            host: cpal::default_host(),
            output_stream: None,
            output_sender: tx,
            output_reciever: Arc::new(rx),
            background_image: if bg_path.try_exists().expect("path exist check failed") {
                Some(image::Handle::from_path(bg_path))
            } else {
                None
            },
        }
    }
}

fn err_fn(err: StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}

fn input_data_fn(data: &[f32], _: &cpal::InputCallbackInfo, tx: mpsc::Sender<Vec<f32>>) {
    let output_data = data.iter().map(|sample| *sample).collect::<Vec<f32>>();

    tx.send(output_data).unwrap();
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        (App::default(), Command::none())
    }

    fn title(&self) -> String {
        "Stream Intro".to_string()
    }

    fn subscription(&self) -> iced_native::Subscription<Self::Message> {
        let events = subscription::events().map(Message::Event);
        let ticks = iced::time::every(std::time::Duration::from_millis(10)).map(|_| Message::Tick);

        Subscription::batch(vec![events, ticks])
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::ShowOutputModal => {
                self.show_output_modal = true;

                let output_devices = self.host.output_devices().unwrap();
                self.output_device_names = output_devices
                    .map(|device| device.name().unwrap())
                    .collect::<Vec<String>>();

                Command::none()
            }
            Message::HideOutputModal => {
                self.hide_modal();
                Command::none()
            }
            Message::Tick => Command::none(),
            Message::SelectedDevice(device) => {
                self.hide_modal();

                if let Some(ref output_stream) = self.output_stream {
                    output_stream.pause().unwrap();

                    let (tx, rx) = mpsc::channel();
                    self.output_sender = tx;
                    self.output_reciever = Arc::new(rx);
                }

                let device = self
                    .host
                    .output_devices()
                    .unwrap()
                    .find(|x| x.name().map(|y| y == device).unwrap_or(false))
                    .expect("failed to find input device {device}");

                let config: cpal::StreamConfig = device.default_input_config().unwrap().into();

                let tx = self.output_sender.clone();

                self.output_stream = Some(
                    device
                        .build_input_stream(
                            &config,
                            move |data: &[f32], cb_info: &cpal::InputCallbackInfo| {
                                let tx = tx.clone();

                                input_data_fn(data, cb_info, tx);
                            },
                            err_fn,
                            None,
                        )
                        .unwrap(),
                );

                if let Some(ref output_stream) = self.output_stream {
                    output_stream.play().unwrap();
                }

                self.page = Page::Waveform;

                self.theme = Theme::custom(theme::Palette {
                    background: Color::from_rgb(0., 1., 0.),
                    ..Theme::Light.palette()
                });

                Command::none()
            }
            Message::Event(event) => match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key_code: keyboard::KeyCode::Tab,
                    modifiers,
                }) => {
                    if modifiers.shift() {
                        widget::focus_previous()
                    } else {
                        widget::focus_next()
                    }
                }
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key_code: keyboard::KeyCode::Escape,
                    ..
                }) => {
                    match self.page {
                        Page::Main => {
                            self.hide_modal();

                            self.theme = Theme::Dark;
                        }
                        Page::Waveform => {
                            self.page = Page::Main;

                            self.theme = Theme::custom(theme::Palette {
                                background: Color::from_rgb(0., 1., 0.),
                                ..Theme::Light.palette()
                            });
                        }
                    }

                    Command::none()
                }
                _ => Command::none(),
            },
        }
    }

    fn view(&self) -> Element<Message> {
        match self.page {
            Page::Main => {
                let content = container(
                    column![
                        row![
                            text("Top Left"),
                            horizontal_space(Length::Fill),
                            text("Top Right"),
                        ]
                        .align_items(Alignment::Start)
                        .height(Length::Fill),
                        container(
                            button(text("Select Output Device")).on_press(Message::ShowOutputModal)
                        )
                        .center_x()
                        .center_y()
                        .width(Length::Fill)
                        .height(Length::Fill),
                        row![
                            text("Bottom Left"),
                            horizontal_space(Length::Fill),
                            text("Bottom Right"),
                        ]
                        .align_items(Alignment::End)
                        .height(Length::Fill)
                    ]
                    .height(Length::Fill),
                )
                .padding(10)
                .width(Length::Fill)
                .height(Length::Fill);

                if self.show_output_modal {
                    let mut output_devices_column =
                        column![text("Output Devices").size(24), horizontal_rule(10)];

                    for output_device_name in &self.output_device_names {
                        if self.output_device_names.first().unwrap() != output_device_name {
                            output_devices_column = output_devices_column.push(vertical_space(10));
                        }

                        output_devices_column = output_devices_column.push(
                            button(text(output_device_name))
                                .width(Length::Fill)
                                .on_press(Message::SelectedDevice(output_device_name.clone())),
                        );
                    }

                    let modal = container(
                        scrollable(output_devices_column)
                            .width(Length::Fill)
                            .id(OUTPUT_SCROLLABLE_ID.clone()),
                    )
                    .width(300)
                    .padding(10)
                    .style(theme::Container::Box);

                    Modal::new(content, modal)
                        .on_blur(Message::HideOutputModal)
                        .into()
                } else {
                    content.into()
                }
            }
            Page::Waveform => {
                let rx = Arc::clone(&self.output_reciever);

                container(
                    Canvas::new(Waveform { rx })
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            }
        }
    }

    fn theme(&self) -> Self::Theme {
        self.theme.clone()
    }
}

impl App {
    fn hide_modal(&mut self) {
        self.show_output_modal = false;
    }
}

fn main() -> iced::Result {
    App::run(Settings::default())
}
