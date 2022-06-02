glslc granite-shaders/bc6.frag -o granite-shaders/bc6.frag.spv
spirv-opt granite-shaders/bc6.frag.spv -O -o granite-shaders/bc6.frag.spv

glslc granite-shaders/fullscreen_tri.vert -o granite-shaders/fullscreen_tri.vert.spv
spirv-opt granite-shaders/fullscreen_tri.vert.spv -O -o granite-shaders/fullscreen_tri.vert.spv