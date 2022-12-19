struct Renderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
}

impl Renderer {
    fn prepare(&self, _device: &wgpu::Device, queue: &wgpu::Queue, angle: f32) {
        puffin::profile_function!(angle.to_string());
        // Update our uniform buffer with the angle from the UI
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[angle]));
    }

    fn paint<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        puffin::profile_function!();
        // Draw our triangle!
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}

