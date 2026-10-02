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
    texture_flags: vec4<f32>,
};

@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var base_color_texture: texture_2d<f32>;
@group(1) @binding(2) var base_color_sampler: sampler;
@group(1) @binding(3) var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(4) var normal_texture: texture_2d<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) joints: vec4<u32>,
    @location(5) weights: vec4<f32>,
    @location(6) model_0: vec4<f32>,
    @location(7) model_1: vec4<f32>,
    @location(8) model_2: vec4<f32>,
    @location(9) model_3: vec4<f32>,
    @location(10) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    output.position = globals.view_projection * model * vec4<f32>(input.position, 1.0);
    output.normal = normalize((model * vec4<f32>(input.normal, 0.0)).xyz);
    output.tangent = vec4<f32>(normalize((model * vec4<f32>(input.tangent.xyz, 0.0)).xyz), input.tangent.w);
    output.color = input.color;
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let diffuse = max(dot(normalize(input.normal), -normalize(globals.light_direction.xyz)), 0.0);
    let mr_sample = textureSample(metallic_roughness_texture, base_color_sampler, input.uv);
    let metallic = select(material.metallic_roughness.x, material.metallic_roughness.x * mr_sample.b, material.texture_flags.y > 0.5);
    let roughness_value = select(material.metallic_roughness.y, material.metallic_roughness.y * mr_sample.g, material.texture_flags.y > 0.5);
    let roughness = clamp(roughness_value, 0.04, 1.0);
    let lighting = globals.ambient.xyz + diffuse * globals.light_color.xyz * globals.light_color.w;
    let sampled = textureSample(base_color_texture, base_color_sampler, input.uv);
    let albedo = mix(material.base_color, material.base_color * sampled, step(0.5, material.texture_flags.x));
    let mapped_normal = textureSample(normal_texture, base_color_sampler, input.uv).xyz * 2.0 - vec3<f32>(1.0);
    let tangent_frame = mat3x3<f32>(normalize(input.tangent.xyz), normalize(cross(input.normal, input.tangent.xyz)) * input.tangent.w, normalize(input.normal));
    let normal = normalize(select(input.normal, tangent_frame * mapped_normal, material.texture_flags.z > 0.5));
    let normal_light = max(dot(normal, -normalize(globals.light_direction.xyz)), 0.0);
    let pbr = (1.0 - 0.15 * metallic) * (0.75 + 0.25 * normal_light) / roughness;
    return vec4<f32>(input.color.xyz * albedo.xyz * (globals.ambient.xyz + normal_light * globals.light_color.xyz * globals.light_color.w) * pbr, input.color.w * albedo.w);
}
