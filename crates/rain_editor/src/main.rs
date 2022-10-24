use std::sync::Arc;

use eframe::{
    egui::{self, LayerId, Id, Ui},
    egui_wgpu::{self, wgpu},
    wgpu::util::DeviceExt,
    Renderer,
};

use egui_dock::{Tree, NodeIndex, DockArea};

struct RainEngine {
    tree: Tree<String>,
    context: TabViewer,
}

struct TabViewer {
    angle: f32,
}

impl egui_dock::TabViewer for TabViewer {
    type Tab = String;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        puffin::profile_function!(&tab);

        match tab.as_str() {
            "Viewer" => {
                //self.custom_painting(ui);
                ui.horizontal(|ui| {

                    if ui.button("run").clicked() {
                        println!("cargo run [Project]")
                    }
                    if ui.button("build 🔨").clicked() {
                        println!("build")
                    }
                });
                egui::Frame::canvas(ui.style()).show(ui, |ui| {
                    self.custom_painting(ui);
                });

            },
            "Editor Debug" => {
                puffin_egui::profiler_ui(ui)
            },
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
        let (rect, response) =
            ui.allocate_at_least(ui.max_rect().size(), egui::Sense::drag());

        self.angle += response.drag_delta().x * 0.01;

        // Clone locals so we can move them into the paint callback:
        let angle = self.angle;

        // The callback function for WGPU is in two stages: prepare, and paint.
        //
        // The prepare callback is called every frame before paint and is given access to the wgpu
        // Device and Queue, which can be used, for instance, to update buffers and uniforms before
        // rendering.
        //
        // The paint callback is called after prepare and is given access to the render pass, which
        // can be used to issue draw commands.
        let cb = egui_wgpu::CallbackFn::new()
            .prepare(move |device, queue, paint_callback_resources| {
                let resources: &TriangleRenderResources = paint_callback_resources.get().unwrap();
                resources.prepare(device, queue, angle);
            })
            .paint(move |_info, rpass, paint_callback_resources| {
                let resources: &TriangleRenderResources = paint_callback_resources.get().unwrap();
                resources.paint(rpass);
            });

        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(cb),
        };

        ui.painter().add(callback);
    }
}

fn main() {
    #[cfg(debug_assertions)]
    puffin::set_scopes_on(true);

    let native_options = eframe::NativeOptions {
        renderer: Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "RainEngine",
        native_options,
        Box::new(|cc| Box::new(RainEngine::new(cc))),
    );
}


impl RainEngine {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        puffin::profile_function!();
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
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
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
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

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0.0]),
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::MAP_WRITE
                | wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Because the graphics pipeline must have the same lifetime as the egui render pass,
        // instead of storing the pipeline in our `Custom3D` struct, we insert it into the
        // `paint_callback_resources` type map, which is stored alongside the render pass.
        wgpu_render_state
            .egui_rpass
            .write()
            .paint_callback_resources
            .insert(TriangleRenderResources {
                pipeline,
                bind_group,
                uniform_buffer,
            });
        let context = TabViewer {
            angle: 1f32
        };

        let mut tree = Tree::new(vec!["Viewer".to_owned(), "tab2".to_owned()]);

        // You can modify the tree before constructing the dock
        let [a, b] = tree.split_left(NodeIndex::root(), 0.2, vec!["Hirachy".to_owned()]);
        let [_, _] = tree.split_below(a, 0.7, vec!["Content browser".to_owned(), "Editor Debug".to_owned()]);
        let [_, _] = tree.split_below(b, 0.65, vec!["tab5".to_owned()]);

        Self {
            tree,
            context,
        }
    }

}

impl Default for RainEngine {
    fn default() -> Self {
        let context = TabViewer {
            angle: 1f32
        };

        let mut tree = Tree::new(vec!["Viewer".to_owned(), "tab2".to_owned()]);

        // You can modify the tree before constructing the dock
        let [a, b] = tree.split_left(NodeIndex::root(), 0.2, vec!["Hirachy".to_owned()]);
        let [_, _] = tree.split_below(a, 0.7, vec!["Content browser".to_owned(), "Editor Debug".to_owned()]);
        let [_, _] = tree.split_below(b, 0.65, vec!["tab5".to_owned()]);

        Self {
            tree,
            context,
        }
    }
}

impl eframe::App for RainEngine {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        puffin::GlobalProfiler::lock().new_frame(); // call once per frame!

        let mut my_frame = egui::containers::Frame::default();
        my_frame.fill = egui::Color32::BLACK;

        egui::TopBottomPanel::top("top_panel").frame(my_frame).show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
                    if ui.button("Quit").clicked() {
                        frame.close();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Settings").clicked() {

                    }
                });

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

        let mut style = egui_dock::Style::from_egui(ctx.style().as_ref());
        style.separator_width = 1.0;
        style.separator_color = egui::Color32::BLACK;
        style.tab_rounding.nw = 7.0;
        style.tab_rounding.ne = 7.0;

        DockArea::new(&mut self.tree)
            .style(style)
            .show_inside(&mut ui, &mut self.context)
    }
}

struct TriangleRenderResources {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}

impl TriangleRenderResources {
    fn prepare(&self, _device: &wgpu::Device, queue: &wgpu::Queue, angle: f32) {
        puffin::profile_function!(angle.to_string());
        // Update our uniform buffer with the angle from the UI
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[angle]));
    }

    fn paint<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        puffin::profile_function!();
        // Draw our triangle!
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}

