glslc granite-shaders/bc6.comp -o granite-shaders/bc6.comp.spv
spirv-opt granite-shaders/bc6.comp.spv -O -o granite-shaders/bc6.comp.spv