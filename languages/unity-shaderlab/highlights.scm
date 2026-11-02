(comment) @comment

(string) @string

(number) @number

(identifier) @variable

[
  "Shader"
  "Properties"
  "SubShader"
  "Pass"
  "Tags"
  "Name"
  "LOD"
  "UsePass"
  "GrabPass"
  "PackageRequirements"
  "CustomEditor"
  "CustomEditorForRenderPipeline"
  "Fallback"
  "Dependency"
  "Stencil"
] @keyword

[
  "HLSLPROGRAM"
  "HLSLINCLUDE"
  "ENDHLSL"
  "CGPROGRAM"
  "CGINCLUDE"
  "ENDCG"
  "GLSLPROGRAM"
  "ENDGLSL"
] @keyword

((fallback
  shader: (string) @constant.builtin)
  (#eq? @constant.builtin "\"Off\""))

(shader
  name: (string) @label)

(name
  (string) @label)

(use_pass
  (string) @label)

(command_name) @constant.builtin

(stencil_field) @property

(render_state
  argument: (identifier) @constant.builtin)

(legacy_command
  argument: (identifier) @constant.builtin)

(stencil_operation
  value: (identifier) @constant.builtin)

(legacy_block
  (identifier) @constant.builtin)

(property
  name: (identifier) @property)

(property_reference
  name: (identifier) @property)

(attribute
  name: (identifier) @attribute)

(tag
  key: (string) @property)

(property_type) @type.builtin

(property_type
  (identifier) @type.builtin)

[
  "Range"
  "Vector"
] @type.builtin
