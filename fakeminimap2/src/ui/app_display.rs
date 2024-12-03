use crossterm::event::{self, Event};
use layout::Flex;
use num_format::SystemLocale;
use num_format::{Locale, ToFormattedString};
use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::prelude::*;
use ratatui::symbols;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType};
use ratatui::widgets::{Block, Cell, Clear, List, ListDirection, ListState, Paragraph, Row, Table};
use ratatui::{prelude::CrosstermBackend, text::Text, Frame, Terminal};
use tachyonfx::fx::parallel;
use tachyonfx::Shader;
use tachyonfx::{fx, fx::Direction, Duration, Effect, EffectRenderer, Interpolation::*};

use crate::state::{self, ui_state};

const COLORS: [Color; 6] = [
    // Pink, Yellow, Brighter Pink, Brighter Yellow, Darker Pink, Darker Yellow
    Color::Rgb(255, 62, 181),
    Color::Rgb(255, 233, 0),
    Color::Rgb(197, 0, 102),
    Color::Rgb(255, 243, 112),
    Color::Rgb(255, 112, 200),
    Color::Rgb(156, 142, 0)
    ];

pub struct AppDisplay {
    state: state::UiState,
    frame_count: usize,
    query_list: ListState,
    dispatcher_tx: tokio::sync::mpsc::UnboundedSender<state::Action>,
    query_list_rect: Rect,

    left_area_effect: Option<Effect>,
    mappings_list_effect: Option<Effect>,
    plot_area_effect: Option<Effect>,

    query_list_height: usize,
    
    pub last_tick: Duration,
    pub plot_data: Vec<Vec<(f64, f64)>>,
}

impl AppDisplay {
    pub fn new(
        state: state::UiState,
        dispatcher_tx: tokio::sync::mpsc::UnboundedSender<state::Action>,
    ) -> Self {
        let query_list_state = ListState::default();
        Self {
            state,
            frame_count: 0,
            query_list: query_list_state,
            dispatcher_tx,
            query_list_rect: Rect::default(),
            left_area_effect: None,
            mappings_list_effect: None,
            plot_area_effect: None,
            last_tick: Duration::ZERO,
            plot_data: Vec::new(),
            query_list_height: 0,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.state.shutdown
    }

    pub fn set_state(&mut self, state: state::UiState) {
        if self.state.selected_query_sequence != state.selected_query_sequence {
            self.mappings_list_effect = None;

            self.plot_area_effect = None;
        }

        self.state = state;
    }

    pub async fn handle_keypress(&mut self, key: event::KeyEvent) {

        // Only if the selected panel is QuerySequences
        if self.state.selected_panel == ui_state::SelectedPanel::QuerySequences {
            // todo, replace with a function
            // PgUp and PgDown
            match key.code {
                event::KeyCode::PageUp => {
                    let new_selection = self.query_list.selected().unwrap_or(0).saturating_sub(self.query_list_height);
                    self.query_list.select(Some(new_selection));
                    self.dispatcher_tx
                        .send(state::Action::SetSelectedQuery(new_selection.into()))
                        .expect("Unable to send selected query sequence");
                    return
                },
                event::KeyCode::PageDown => {
                    let new_selection = self.query_list.selected().unwrap_or(0) + self.query_list_height;
                    self.query_list.select(Some(new_selection));
                    self.dispatcher_tx
                        .send(state::Action::SetSelectedQuery(new_selection.into()))
                        .expect("Unable to send selected query sequence");
                    return
                },
                // Home and End Keys
                event::KeyCode::Home => {
                    self.query_list.select(Some(0));
                    self.dispatcher_tx
                        .send(state::Action::SetSelectedQuery(0))
                        .expect("Unable to send selected query sequence");
                    return
                },
                event::KeyCode::End => {
                    let new_selection = self.state.query_sequences_list.len().saturating_sub(1);
                    self.query_list.select(Some(new_selection));
                    self.dispatcher_tx
                        .send(state::Action::SetSelectedQuery(new_selection))
                        .expect("Unable to send selected query sequence");
                    return
                },
                _ => (),
            }
        }

        // Otherwise bubble it to the dispatcher
        self.dispatcher_tx
            .send(state::Action::KeyPress(key))
            .expect("Unable to send key press event");
        
    }

    pub fn handle_click(&mut self, col: u16, row: u16) {
        if self.query_list_rect.contains(Position { x: col, y: row }) {
            // Calculate offset from the top of the list
            let offset = row.saturating_sub(self.query_list_rect.y.saturating_sub(1));

            if offset >= self.state.query_sequences_list.len() as u16 {
                return;
            }

            self.dispatcher_tx
                .send(state::Action::SetSelectedQuery(offset.into()))
                .expect("Unable to send selected query sequence");
        }
    }

    pub fn prepare(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) {
        let screen_bg = Color::Rgb(0, 0, 0);

        terminal
            .draw(|frame| {
                Clear.render(frame.area(), frame.buffer_mut());
                Block::default()
                    .style(Style::default().bg(screen_bg))
                    .render(frame.area(), frame.buffer_mut());
            })
            .expect("Error preparing");
    }

    pub async fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) {
        self.frame_count += 1;
        let locale = SystemLocale::default().unwrap();

        let mappings = if let Some(mappings) = &self.state.mappings {
            Some(mappings.lock().await)
        } else {
            None
        };

        terminal
            .draw(|frame| {
                let screen_bg = Color::Rgb(0, 0, 0);

                let vertical = Layout::vertical([Length(1), Min(0), Length(1)]);
                let [title_area, main_area, status_area] = vertical.areas(frame.area());
                let horizontal = Layout::horizontal([Fill(1), Fill(2)]);
                let [left_area, right_area] = horizontal.areas(main_area);

                let mut count = 0;

                let border_style_default = Style::default().fg(Color::White);
                let border_style_selected = Style::default().fg(Color::Yellow);

                let query_sequences_border_style =
                    if self.state.selected_panel == ui_state::SelectedPanel::QuerySequences {
                        border_style_selected
                    } else {
                        border_style_default
                    };

                let mappings_border_style =
                    if self.state.selected_panel == ui_state::SelectedPanel::Mappings {
                        border_style_selected
                    } else {
                        border_style_default
                    };

                // Left area list of query sequence names
                if self.state.query_sequences_list.is_empty() {
                    let text = Text::styled("Indexing Reference", Style::default().fg(Color::LightRed))
                        .bg(screen_bg);
                    let para = Paragraph::new(text)
                        .block(
                            Block::default().style(Style::default().bg(screen_bg))
                        )
                        .alignment(Alignment::Center);

                    frame.render_widget(para, left_area);

                    self.query_list_rect = left_area;
                } else {
                    let mut items = vec![];
                    let selected;
                    for (i, query) in self.state.query_sequences_list.iter().enumerate() {
                        if self.state.selected_query_sequence.is_some()
                            && query.id == self.state.selected_query_sequence.as_ref().unwrap().id
                        {
                            self.query_list.select(Some(i));
                        }
                        items.push(query.id.clone());
                    }
                    count = self.state.query_sequences_list.len();

                    if self.query_list.selected().is_none() {
                        self.query_list.select_first();
                        self.dispatcher_tx
                            .send(state::Action::SetSelectedQuery(
                                self.query_list.selected().unwrap(),
                            ))
                            .expect("Unable to send selected query sequence");
                    }

                    selected = self.query_list.selected().unwrap_or(0);

                    let list = List::new(items)
                        .block(
                            Block::bordered()
                                .title("Query Sequences")
                                .style(Style::default().bg(screen_bg))
                                .border_style(query_sequences_border_style)
                                .title_bottom(format!("Query Sequences {}/{}", selected, count))
                        )
                        .style(Style::new().white())
                        .highlight_style(Style::new().italic())
                        .highlight_symbol(">>")
                        .repeat_highlight_symbol(true)
                        .bg(screen_bg)
                        .direction(ListDirection::TopToBottom);

                    frame.render_stateful_widget(list, left_area, &mut self.query_list);
                    self.query_list_rect = left_area;

                    // How many query sequences are shown on a single screen?
                    let query_list_height = left_area.height as usize - 2;
                    self.query_list_height = query_list_height;


                    /*
                    if self.left_area_effect.is_none() {
                        self.left_area_effect = Some(fx::sweep_in(
                            Direction::LeftToRight,
                            30,
                            0,
                            screen_bg,
                            (Duration::from_millis(1250), QuadOut),
                        ).with_area(left_area));
                    }

                    if let Some(effect) = self.left_area_effect.as_mut() {
                        frame.render_effect(effect, left_area, self.last_tick);
                    } 
                    */
                }

                // Render the mappings
                if let Some(mappings) = mappings {
                    let mut datasets = Vec::new();
                    self.plot_data.clear();

                    let (mut x_lower_bound, mut x_upper_bound) = (u64::MAX, u64::MIN);
                    let (mut y_lower_bound, mut y_upper_bound) = (u64::MAX, u64::MIN);

                    // todo - need more separation between different target sequences

                    let rows = mappings
                        .iter()
                        .enumerate()
                        .map(|(i, mapping)| {
                            x_lower_bound = x_lower_bound.min(mapping.query_start as u64);
                            x_upper_bound = x_upper_bound.max(mapping.query_end as u64);
                            y_lower_bound = y_lower_bound.min(mapping.target_start as u64);
                            y_upper_bound = y_upper_bound.max(mapping.target_end as u64);

                            let data = vec![
                                (mapping.query_start as f64, mapping.target_start as f64),
                                (mapping.query_end as f64, mapping.target_end as f64),
                            ];

                            self.plot_data.push(data.clone());

                            let text = mapping
                                .target_name
                                .as_ref()
                                .map(|s| s.to_string())
                                .unwrap_or("".to_string());

                            let text = Text::styled(text, Style::default().fg(COLORS[i % COLORS.len()]));

                            Row::new(vec![
                                Cell::from(text),
                                Cell::from(mapping.query_start.to_formatted_string(&locale)),
                                Cell::from(mapping.query_end.to_formatted_string(&locale)),
                                Cell::from(mapping.target_start.to_formatted_string(&locale)),
                                Cell::from(mapping.target_end.to_formatted_string(&locale)),
                                Cell::from(mapping.match_len.to_formatted_string(&locale)),
                            ]).style(Style::default().bg(screen_bg).fg(COLORS[i % COLORS.len()]))
                        })
                        .collect::<Vec<_>>();

                    let widths = [
                        Constraint::Fill(2),
                        Constraint::Fill(1),
                        Constraint::Fill(1),
                        Constraint::Fill(1),
                        Constraint::Fill(1),
                        Constraint::Fill(1),
                    ];

                    let table = Table::new(rows, widths)
                        .column_spacing(2)
                        .header(
                            Row::new(vec![
                                Cell::from("Target"),
                                Cell::from("qStart"),
                                Cell::from("qEnd"),
                                Cell::from("tStart"),
                                Cell::from("tEnd"),
                                Cell::from("Match Len"),
                            ])
                            .style(Style::new().bold())
                            .bottom_margin(1),
                        )
                        .style(Style::default().bg(screen_bg))  
                        .flex(Flex::SpaceAround)
                        .block(
                            Block::bordered()
                                .title("Mappings")
                                .border_style(mappings_border_style)
                                .title_bottom(format!("{} Mappings", mappings.len())),
                        );

                    let vertical = Layout::vertical([Fill(2), Fill(6)]);
                    let [top_area, plot_area] = vertical.areas(right_area);

                    frame.render_widget(table, top_area);

                    /*
                    if self.mappings_list_effect.is_none() {
                        self.mappings_list_effect = Some(fx::hsl_shift_fg(
                            [360.0, 0.0, 0.0],
                            Duration::from_millis(350),
                        ));
                        // Let's do a sweep in effect
                        self.mappings_list_effect = Some(fx::sweep_in(
                            Direction::RightToLeft,
                            4,
                            128,
                            screen_bg,
                            (Duration::from_millis(1250), QuadOut),
                        ).with_area(top_area));
                    }  */

                    self.plot_data.iter().enumerate().for_each(|(i, data)| {
                        let dataset = Dataset::default()
                            // .name(format!("Mapping {}", i))
                            .graph_type(GraphType::Line)
                            .marker(symbols::Marker::Braille)
                            .data(&data)
                            .style(Style::default().fg(COLORS[i % COLORS.len()]));

                        datasets.push(dataset);
                    });

                    let x_axis = Axis::default()
                        .title("Query".red())
                        .style(Style::default().white())
                        .bounds([x_lower_bound as f64, x_upper_bound as f64]);

                    let y_axis = Axis::default()
                        .title("Target".blue())
                        .style(Style::default().white())
                        .bounds([y_lower_bound as f64, y_upper_bound as f64]);

                    let chart = Chart::new(datasets)
                        .block(Block::new().title("Chart").style(Style::default().bg(screen_bg)))
                        .x_axis(x_axis)
                        .y_axis(y_axis)
                        .style(Style::default().bg(screen_bg));

                    /* 
                    if self.plot_area_effect.is_none() {
                        self.plot_area_effect = Some(fx::sweep_in(
                            Direction::UpToDown,
                            6,
                            128,
                            COLORS[1],
                            Duration::from_millis(550),
                        ).with_area(plot_area));
                    }
                    */

                    frame.render_widget(chart, plot_area);

                    /*
                    if let Some(effect) = self.mappings_list_effect.as_mut() {
                        frame.render_effect(effect, top_area, self.last_tick);
                    } */

                } else {
                    frame.render_widget(
                        Text::styled("No mappings", Style::default().fg(Color::Red)),
                        right_area,
                    );
                }

                frame.render_widget(Block::bordered().title("Fakeminimap2"), title_area);

                frame.render_widget(
                    Block::bordered().title(format!(
                        "{} - {} Entries - {} Frame Renders",
                        self.state.status, count, self.frame_count
                    )),
                    status_area,
                );
            })
            .expect("Error rendering");
    }
}
