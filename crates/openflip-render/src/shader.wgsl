// Draws instanced quads: rounded rectangles (mode 0), optionally with a felt
// texture, and glyphs from the atlas (mode 1). Any quad can be folded: cut
// at a hinge line, keeping one half, and rotated about that hinge with
// perspective, which is how the flaps of the flip clock move.

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
    // glyphs: atlas rectangle in pixels (x, y, width, height);
    // rectangles: texture strength, 1 if the target is sRGB (linear values)
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
    // corner radius, mode
    @location(3) params: vec2<f32>,
    // hinge y, centre x, half (-1 top, 1 bottom, 0 none), angle
    @location(4) fold: vec4<f32>,
    // eye distance (0 = not rotated), shade, shade falloff, unused
    @location(5) fold_extra: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) @interpolate(flat) params: vec2<f32>,
    // distance from the hinge before rotation, perspective-correct
    @location(5) hinge_offset: f32,
    // half, shade, shade falloff
    @location(6) @interpolate(flat) fold: vec3<f32>,
    // rectangles: top-left corner, texture strength, linear target
    @location(7) @interpolate(flat) surface: vec4<f32>,
};

fn to_ndc(pixel: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        pixel.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / globals.viewport.y * 2.0,
    );
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32, instance: Instance) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[index];
    let pixel = instance.rect.xy + corner * instance.rect.zw;

    let hinge = instance.fold.x;
    let dy = pixel.y - hinge;
    let eye = instance.fold_extra.x;

    var projected = to_ndc(pixel);
    var w = 1.0;
    if eye > 0.0 {
        // Rotate about the hinge (x axis); z points towards the viewer.
        let angle = instance.fold.w;
        let rotated = vec2<f32>(pixel.x, hinge + dy * cos(angle));
        let z = -dy * sin(angle);
        w = (eye - z) / eye;
        // Perspective around the card's centre on the hinge: after the
        // division by w, points move away from it as they come closer.
        let center = to_ndc(vec2<f32>(instance.fold.y, hinge));
        projected = center * w + (to_ndc(rotated) - center);
    }

    var out: VertexOutput;
    out.position = vec4<f32>(projected, 0.0, w);
    out.local = corner * instance.rect.zw;
    out.size = instance.rect.zw;
    out.uv = (instance.uv.xy + corner * instance.uv.zw) / globals.atlas_size;
    out.color = instance.color;
    out.params = instance.params;
    out.hinge_offset = dy;
    out.fold = vec3<f32>(instance.fold.z, instance.fold_extra.y, instance.fold_extra.z);
    out.surface = vec4<f32>(instance.rect.xy, instance.uv.xy);
    return out;
}

// Felt texture: fine grain with a hint of unevenness, in sRGB units around
// zero. `p` is a position on the rectangle at rest, so the
// pattern sticks to it while it folds; `scale` follows its size so the felt
// looks the same at any resolution.

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn felt(p: vec2<f32>, scale: f32) -> f32 {
    let grain = hash(floor(p)) - 0.5;
    let coarse = value_noise(p / 2.5) - 0.5;
    let unevenness = value_noise(p / (scale * 0.05)) - 0.5;
    return grain * 0.03 + coarse * 0.016 + unevenness * 0.006;
}

// A soft light facing the screen: brightest in the middle of the screen,
// fading out gently towards the edges.
fn light(p: vec2<f32>) -> f32 {
    let centre = globals.viewport * 0.5;
    let reach = max(globals.viewport.x, globals.viewport.y) * 0.4;
    let d = length(p - centre) / reach;
    return exp(-d * d);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Keep only the requested half of a folded quad.
    let half = in.fold.x;
    if (half < 0.0 && in.hinge_offset > 0.0) || (half > 0.0 && in.hinge_offset < 0.0) {
        discard;
    }

    var shade = in.fold.y;
    if in.fold.z > 0.0 {
        shade = shade * clamp(1.0 - abs(in.hinge_offset) / in.fold.z, 0.0, 1.0);
    }
    let tint = 1.0 - shade;

    if in.params.y > 0.5 {
        let coverage = textureSampleLevel(atlas, atlas_sampler, in.uv, 0.0).r;
        return vec4<f32>(in.color.rgb * tint, in.color.a * coverage);
    }
    // Signed distance to a rounded rectangle, anti-aliased over one pixel.
    let half_size = in.size * 0.5;
    let radius = min(in.params.x, min(half_size.x, half_size.y));
    let q = abs(in.local - half_size) - half_size + vec2<f32>(radius);
    let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
    let alpha = clamp(0.5 - distance, 0.0, 1.0);
    var color = in.color.rgb;
    let texture = in.surface.z;
    if texture > 0.0 {
        let linear = in.surface.w > 0.5;
        var srgb = color;
        if linear {
            srgb = linear_to_srgb(color);
        }
        // The light brightens the card and, more so, brings out its felt.
        let p = in.surface.xy + in.local;
        let lit = texture * light(p);
        srgb = srgb * (1.0 + 1.0 * lit) + vec3<f32>(texture * (1.0 + 1.5 * lit) * felt(p, in.size.y));
        color = select(srgb, srgb_to_linear(srgb), linear);
    }
    return vec4<f32>(color * tint, in.color.a * alpha);
}
