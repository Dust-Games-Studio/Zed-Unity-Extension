"xml" @keyword

[ "version" "encoding" "standalone" ] @property

(EncName) @string.special
(VersionNum) @number
[ "yes" "no" ] @boolean

(PI) @embedded
(PI (PITarget) @keyword)

(STag (Name) @tag)
(ETag (Name) @tag)
(EmptyElemTag (Name) @tag)

(Attribute (Name) @property)
(Attribute (AttValue) @string)

[
  "<?" "?>"
  "<!" "]]>"
  "<" ">"
  "</" "/>"
] @punctuation.delimiter

[ "(" ")" "[" "]" ] @punctuation.bracket
[ "\"" "'" ] @punctuation.delimiter
[ "," "|" "=" ] @operator

(CharData) @markup
(CDSect
  (CDStart) @markup.heading
  (CData) @markup.raw
  "]]>" @markup.heading)
(Comment) @comment
(ERROR) @error

((STag (Name) @type.builtin)
  (#match? @type.builtin "^(ui:|uie:)?(UXML|VisualElement|Button|Label|TextField|Toggle|Slider|SliderInt|MinMaxSlider|ProgressBar|ScrollView|ListView|TreeView|MultiColumnListView|MultiColumnTreeView|Foldout|Box|Image|IMGUIContainer|Template|Instance|Style|AttributeOverrides|Bindings|Binding|DataBinding|Editor|Toolbar|ToolbarButton|ToolbarToggle|ToolbarMenu|ToolbarSearchField|ToolbarBreadcrumbs|DropdownField|EnumField|ObjectField|ColorField|CurveField|GradientField|LayerMaskField|MaskField|TagField|PropertyField|HelpBox|RadioButton|RadioButtonGroup|GroupBox|PopupWindow|Window|TextElement)$"))

((EmptyElemTag (Name) @type.builtin)
  (#match? @type.builtin "^(ui:|uie:)?(UXML|VisualElement|Button|Label|TextField|Toggle|Slider|SliderInt|MinMaxSlider|ProgressBar|ScrollView|ListView|TreeView|MultiColumnListView|MultiColumnTreeView|Foldout|Box|Image|IMGUIContainer|Template|Instance|Style|AttributeOverrides|Bindings|Binding|DataBinding|Editor|Toolbar|ToolbarButton|ToolbarToggle|ToolbarMenu|ToolbarSearchField|ToolbarBreadcrumbs|DropdownField|EnumField|ObjectField|ColorField|CurveField|GradientField|LayerMaskField|MaskField|TagField|PropertyField|HelpBox|RadioButton|RadioButtonGroup|GroupBox|PopupWindow|Window|TextElement)$"))

((Attribute (Name) @attribute)
  (#match? @attribute "^(name|class|style|text|binding-path|data-source-path|data-source-type|picking-mode|usage-hints|tooltip|focusable|tabindex|enabled|visible|content-container|xmlns|xmlns:ui|xmlns:uie|xmlns:xsi)$"))
