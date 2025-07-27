#version 460 core

out vec4 frag_color;

void main() {
    const float normalized_depth = clamp(gl_FragDepth / 100.0, 0.0, 1.0);
    frag_color = normalize(vec4(normalized_depth, normalized_depth, normalized_depth, 1));
}
