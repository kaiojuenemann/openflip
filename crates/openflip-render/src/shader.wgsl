// Draws instanced quads: rounded rectangles (mode 0) and glyphs from the atlas (mode 1).

struct Globals {
    viewport: vec2<f32>,
    atlas_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct Instance {
    // x, y, width, height in pixels
    @location(0) rect: vec4<f32>,
    // atlas rectangle in pixels: x, y, width, height
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
    // corner radius, mode
    @location(3) params: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) @interpolate(flat) params: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32, instance: Instance) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[index];
    let pixel = instance.rect.xy + corner * instance.rect.zw;

    var out: VertexOutput;
    out.position = vec4<f32>(
        pixel.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / globals.viewport.y * 2.0,
        0.0,
        1.0,
    );
    out.local = corner * instance.rect.zw;
    out.size = instance.rect.zw;
    out.uv = (instance.uv.xy + corner * instance.uv.zw) / globals.atlas_size;
    out.color = instance.color;
    out.params = instance.params;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.params.y > 0.5 {
        let coverage = textureSampleLevel(atlas, atlas_sampler, in.uv, 0.0).r;
        return vec4<f32>(in.color.rgb, in.color.a * coverage);
    }
    // Signed distance to a rounded rectangle, anti-aliased over one pixel.
    let half_size = in.size * 0.5;
    let radius = min(in.params.x, min(half_size.x, half_size.y));
    let q = abs(in.local - half_size) - half_size + vec2<f32>(radius);
    let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
    let alpha = clamp(0.5 - distance, 0.0, 1.0);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
