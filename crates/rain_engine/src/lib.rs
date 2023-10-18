#[allow(dead_code)]
struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    renderer: cloudy_renderer::CloudyRenderer,
}
