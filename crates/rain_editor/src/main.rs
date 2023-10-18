use cloudy_renderer::CloudyRenderer;
use std::sync::Arc;

use eframe::{
    egui::{self, Id, LayerId, Ui},
    egui_wgpu::{self, wgpu},
    wgpu::util::DeviceExt,
    Renderer,
};

use egui_dock::{DockArea, DockState, NodeIndex};

struct RainEngine {
    tree: DockState<String>,
    context: TabViewer,
}

struct EditorTabCallback {
    angle: f32,
}

impl egui_wgpu::CallbackTrait for EditorTabCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources: &CloudyRenderer = resources.get().unwrap();
        resources.prepare(device, queue, self.angle);
        Vec::new()
    }

    fn paint<'a>(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'a>,
        resources: &'a egui_wgpu::CallbackResources,
    ) {
        let resources: &CloudyRenderer = resources.get().unwrap();
        resources.paint(render_pass);
    }
}

struct TabViewer {
    angle: f32,
}

impl egui_dock::TabViewer for TabViewer {
    type Tab = String;
    fn add_popup(&mut self, ui: &mut Ui, _surface: egui_dock::SurfaceIndex, _node: NodeIndex) {
        ui.set_min_width(120.0);
        if ui.button("Regular tab").clicked() {
            println!("Hello")
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        puffin::profile_function!(&tab);

        match tab.as_str() {
            "Viewer" => self.viewer_tab(ui),
            "Editor Debug" => {
                puffin_egui::profiler_ui(ui) // somehow crashes when not in ScrollArea
            }

            "Log" => egui_logger::logger_ui(ui),
            _ => {
                ui.label(format!("Content of {tab}"));
            }
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        (&*tab).into()
    }
}

impl TabViewer {
    fn custom_painting(&mut self, ui: &mut egui::Ui) {
        puffin::profile_function!();

        // Clone locals so we can move them into the paint callback:

        // The callback function for WGPU is in two stages: prepare, and paint.
        //
        // The prepare callback is called every frame before paint and is given access to the wgpu
        // Device and Queue, which can be used, for instance, to update buffers and uniforms before
        // rendering.
        //
        // The paint callback is called after prepare and is given access to the render pass, which
        // can be used to issue draw commands.
        let (rect, response) = ui.allocate_at_least(ui.max_rect().size(), egui::Sense::drag());

        self.angle += response.drag_delta().x * 0.01;
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            EditorTabCallback { angle: self.angle },
        ));
    }

    fn viewer_tab(&mut self, ui: &mut egui::Ui) {
        //self.custom_painting(ui);
        ui.horizontal(|ui| {
            if ui.button("run").clicked() {
                log::info!("run")
            }
            if ui.button("build 🔨").clicked() {
                log::info!("build")
            }
        });
        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            self.custom_painting(ui);
        });
    }
}

fn main() {
    egui_logger::init().expect("Error initializing logger");

    //#[cfg(debug_assertions)]
    puffin::set_scopes_on(true);

    let native_options = eframe::NativeOptions {
        renderer: Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "RainEngine",
        native_options,
        Box::new(|cc| Box::new(RainEngine::new(cc))),
    )
    .unwrap();
}

impl RainEngine {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        puffin::profile_function!();

        let wgpu_render_state = cc.wgpu_render_state.as_ref().expect("WGPU enabled");

        let device = &wgpu_render_state.device;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("./hi.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main thing"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[cloudy_renderer::Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu_render_state.target_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(cloudy_renderer::VERTICES),
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        // Because the graphics pipeline must have the same lifetime as the egui render pass,
        // instead of storing the pipeline in our `Custom3D` struct, we insert it into the
        // `paint_callback_resources` type map, which is stored alongside the render pass.
        wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .insert(CloudyRenderer {
                pipeline,
                bind_group,
                buffer,
            });

        Self::default()
    }
}

impl Default for RainEngine {
    fn default() -> Self {
        let context = TabViewer { angle: 1f32 };

        let mut tree = DockState::new(vec!["Viewer".to_owned(), "tab2".to_owned()]);

        // You can modify the tree before constructing the dock
        let [a, b] =
            tree.main_surface_mut()
                .split_left(NodeIndex::root(), 0.2, vec!["Hirachy".to_owned()]);
        let [_, _] = tree.main_surface_mut().split_below(
            a,
            0.7,
            vec!["Content browser".to_owned(), "Editor Debug".to_owned()],
        );

        //#[cfg(debug_assertions)]
        let [_, _] = tree
            .main_surface_mut()
            .split_below(b, 0.65, vec!["Log".to_owned()]);

        Self { tree, context }
    }
}

impl eframe::App for RainEngine {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        puffin::GlobalProfiler::lock().new_frame(); // call once per frame!

        let mut my_frame = egui::containers::Frame::default();
        my_frame.fill = egui::Color32::BLACK;

        egui::TopBottomPanel::top("top_panel")
            .frame(my_frame)
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
                        if ui.button("Quit").clicked() {
                            frame.close();
                        }
                    });

                    ui.menu_button("Edit", |ui| if ui.button("Settings").clicked() {});

                    ui.menu_button("View", |ui| {
                        if ui.button("Reset window position").clicked() {
                            *self = RainEngine::default()
                        }
                    });

                    ui.menu_button("Help", |ui| {
                        if ui.button("About RainEngine").clicked() {
                            egui::Window::new("about").show(ctx, |ui| {
                                ui.heading("RainEngine");
                                ui.label("Author: Jacob");
                            });
                        }
                    });
                });
            });
        let layer_id = LayerId::background();
        let max_rect = ctx.available_rect();
        let clip_rect = ctx.available_rect();
        let id = Id::new("egui_dock::DockArea");
        let mut ui = Ui::new(ctx.clone(), layer_id, id, max_rect, clip_rect);

        /*
        egui::Window::new("hi").show(ctx, |ui| {
            egui::Frame::canvas(ui.style()).show(ui, |ui| {
                self.custom_painting(ui);
            });
        });
        */

        let style = egui_dock::Style::from_egui(ctx.style().as_ref());

        DockArea::new(&mut self.tree)
            .style(style)
            .show_inside(&mut ui, &mut self.context)
    }
}
