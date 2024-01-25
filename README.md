# Heatwave
This is the repository for Heatwave, a library designed to speed up the production of GPU-based applications, such as games, UIs, simulations and the like.

Heatwave is a minimal library, meaning you as the developer will have to provide most of the rendering or physics work. All Heatwave does is expose the GPU via [WGPU](https://github.com/gfx-rs/wgpu) and handle user events, displaying the surface onto the generated window.
