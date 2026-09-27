struct Globals {
    view_projection: mat4x4<f32>,
    light_direction: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct Material {
    base_color: vec4<f32>,
    metallic_roughness: vec4<f32>,
};

@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var base_color_texture: texture_2d<f32>;
@group(1) @binding(2) var base_color_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(4) model_0: vec4<f32>,
    @location(5) model_1: vec4<f32>,
    @location(6) model_2: vec4<f32>,
    @location(7) model_3: vec4<f32>,
    @location(8) color: vec4<f32>,
    @location(3) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    output.position = globals.view_projection * model * vec4<f32>(input.position, 1.0);
    output.normal = normalize((model * vec4<f32>(input.normal, 0.0)).xyz);
    output.color = input.color;
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let diffuse = max(dot(normalize(input.normal), -normalize(globals.light_direction.xyz)), 0.0);
    let roughness = clamp(material.metallic_roughness.y, 0.04, 1.0);
    let lighting = globals.ambient.xyz + diffuse * globals.light_color.xyz * globals.light_color.w;
    let sampled = textureSample(base_color_texture, base_color_sampler, input.uv);
    let albedo = mix(material.base_color, material.base_color * sampled, material.metallic_roughness.w);
    return vec4<f32>(input.color.xyz * albedo.xyz * lighting * (1.0 - 0.15 * material.metallic_roughness.x) / roughness, input.color.w * albedo.w);
}
