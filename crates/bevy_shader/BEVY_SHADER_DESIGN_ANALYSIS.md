# bevy_shader 库设计思想分析

## 一、架构概览

`bevy_shader` 是 Bevy 引擎的着色器管理库，负责着色器的加载、缓存、预处理和编译。它是连接用户着色器代码与 GPU 渲染管线的桥梁。

### 核心模块结构

```
bevy_shader/
├── src/
│   ├── lib.rs                    # 模块入口和宏定义
│   ├── shader.rs                 # Shader 资源定义和加载器
│   └── shader_cache.rs           # 着色器缓存系统
```

---

## 二、核心设计思想

### 1. **多格式支持**

**设计理念**：支持多种着色器格式，满足不同的开发需求。

**实现机制**：

```rust
#[derive(Debug, Clone)]
pub enum Source {
    /// WebGPU Shading Language (WGSL) - Bevy 推荐格式
    Wgsl(Cow<'static, str>),
    /// Wesl - Bevy 扩展的着色器语言
    Wesl(Cow<'static, str>),
    /// OpenGL Shading Language (GLSL)
    Glsl(Cow<'static, str>, naga::ShaderStage),
    /// SPIR-V 二进制格式
    SpirV(Cow<'static, [u8]>),
}

impl Shader {
    // WGSL 构造函数
    pub fn from_wgsl(source: impl Into<Cow<'static, str>>, path: impl Into<String>) -> Shader {
        let source = source.into();
        let path = path.into();
        let (import_path, imports) = Shader::preprocess(&source, &path);
        Shader {
            path,
            imports,
            import_path,
            source: Source::Wgsl(source),
            additional_imports: Default::default(),
            shader_defs: Default::default(),
            file_dependencies: Default::default(),
            validate_shader: ValidateShader::Disabled,
        }
    }
    
    // 带宏定义的 WGSL
    pub fn from_wgsl_with_defs(
        source: impl Into<Cow<'static, str>>,
        path: impl Into<String>,
        shader_defs: Vec<ShaderDefVal>,
    ) -> Shader {
        Self {
            shader_defs,
            ..Self::from_wgsl(source, path)
        }
    }
    
    // GLSL 构造函数
    pub fn from_glsl(
        source: impl Into<Cow<'static, str>>,
        stage: naga::ShaderStage,
        path: impl Into<String>,
    ) -> Shader {
        let source = source.into();
        let path = path.into();
        let (import_path, imports) = Shader::preprocess(&source, &path);
        Shader {
            path,
            imports,
            import_path,
            source: Source::Glsl(source, stage),
            additional_imports: Default::default(),
            shader_defs: Default::default(),
            file_dependencies: Default::default(),
            validate_shader: ValidateShader::Disabled,
        }
    }
    
    // SPIR-V 构造函数
    pub fn from_spirv(source: impl Into<Cow<'static, [u8]>>, path: impl Into<String>) -> Shader {
        let path = path.into();
        Shader {
            path: path.clone(),
            imports: Vec::new(),
            import_path: ShaderImport::AssetPath(path),
            source: Source::SpirV(source.into()),
            additional_imports: Default::default(),
            shader_defs: Default::default(),
            file_dependencies: Default::default(),
            validate_shader: ValidateShader::Disabled,
        }
    }
}
```

**格式对比**：

| 格式 | 优势 | 劣势 | 适用场景 |
|------|------|------|----------|
| **WGSL** | 原生 WebGPU，语法简洁，类型安全 | 较新，社区资源少 | Bevy 推荐格式 |
| **GLSL** | 成熟，社区资源丰富 | 需要转换 | 移植现有项目 |
| **SPIR-V** | 二进制，加载快，跨平台 | 不可读，调试困难 | 发布版本 |
| **Wesl** | 扩展功能，更强大 | 实验性，不稳定 | 高级特性 |

---

### 2. **预处理系统**

**设计理念**：支持 `#import` 预处理指令，实现模块化着色器开发。

**实现机制**：

```rust
impl Shader {
    /// 预处理着色器源，提取 import 信息
    fn preprocess(source: &str, path: &str) -> (ShaderImport, Vec<ShaderImport>) {
        // 使用 naga_oil 提取预处理数据
        let (import_path, imports, _) = naga_oil::compose::get_preprocessor_data(source);

        // 处理 import_path
        let import_path = import_path
            .map(ShaderImport::Custom)
            .unwrap_or_else(|| ShaderImport::AssetPath(path.to_owned()));

        // 处理 imports
        let imports = imports
            .into_iter()
            .map(|import| {
                if import.import.starts_with('"') {
                    // 资产路径导入
                    let import = import
                        .import
                        .chars()
                        .skip(1)
                        .take_while(|c| *c != '"')
                        .collect();
                    ShaderImport::AssetPath(import)
                } else {
                    // 自定义导入
                    ShaderImport::Custom(import.import)
                }
            })
            .collect();

        (import_path, imports)
    }
}

/// 着色器导入类型
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ShaderImport {
    /// 资产路径导入（相对于 assets 目录）
    AssetPath(String),
    /// 自定义导入（如内置库）
    Custom(String),
}
```

**使用示例**：

```wgsl
// main.wgsl
#import "common/constants.wgsl"
#import "lighting/pbr.wgsl"

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    // 使用导入的常量
    let position = in.position * WORLD_SCALE;
    // ...
}
```

**优势**：
- ✅ 代码复用：共享常量、函数、结构体
- ✅ 模块化：按功能组织着色器代码
- ✅ 依赖管理：自动解析和加载依赖

---

### 3. **着色器宏定义系统**

**设计理念**：支持运行时宏定义，实现条件编译和参数化着色器。

**实现机制**：

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Debug, Hash)]
pub enum ShaderDefVal {
    /// 布尔宏定义
    Bool(String, bool),
    /// 整数宏定义
    Int(String, i32),
    /// 无符号整数宏定义
    UInt(String, u32),
}

impl From<&str> for ShaderDefVal {
    fn from(key: &str) -> Self {
        ShaderDefVal::Bool(key.to_string(), true)
    }
}

impl From<String> for ShaderDefVal {
    fn from(key: String) -> Self {
        ShaderDefVal::Bool(key, true)
    }
}

impl ShaderDefVal {
    pub fn value_as_string(&self) -> String {
        match self {
            ShaderDefVal::Bool(_, def) => def.to_string(),
            ShaderDefVal::Int(_, def) => def.to_string(),
            ShaderDefVal::UInt(_, def) => def.to_string(),
        }
    }
}
```

**使用示例**：

```rust
// 创建带宏定义的着色器
let shader = Shader::from_wgsl_with_defs(
    r#"
        #if TONEMAP_IN_SHADER
            // 色调映射代码
        #endif
        
        @fragment
        fn fragment(in: VertexInput) -> @location(0) vec4<f32> {
            #if DEBAND_DITHER
                // 去色带抖动代码
            #endif
            // ...
        }
    "#,
    "shaders/sprite.wgsl",
    vec![
        ShaderDefVal::Bool("TONEMAP_IN_SHADER".into(), true),
        ShaderDefVal::Bool("DEBAND_DITHER".into(), true),
        ShaderDefVal::UInt("TONEMAPPING_LUT_TEXTURE_BINDING_INDEX".into(), 1),
    ],
);
```

**在着色器缓存中的应用**：

```rust
pub struct ShaderCache<ShaderModule, RenderDevice> {
    // ...
    
    pub fn get(
        &mut self,
        render_device: &RenderDevice,
        pipeline: CachedPipelineId,
        id: AssetId<Shader>,
        shader_defs: &[ShaderDefVal],
    ) -> Result<Arc<ShaderModule>, ShaderCacheError> {
        // ...
        
        // 按宏定义缓存处理后的着色器
        let module = match data.processed_shaders.entry_ref(shader_defs) {
            EntryRef::Occupied(entry) => entry.into_mut(), // 缓存命中
            EntryRef::Vacant(entry) => {
                // 缓存未命中，处理并缓存
                let processed_module = self.process_shader(shader, shader_defs)?;
                entry.insert(processed_module)
            }
        };
        
        Ok(module.clone())
    }
}
```

**优势**：
- ✅ 条件编译：根据宏定义启用/禁用功能
- ✅ 参数化着色器：运行时配置着色器行为
- ✅ 缓存优化：不同宏定义产生不同的缓存条目
- ✅ 减少重复代码：一个着色器模板生成多个变体

---

### 4. **着色器缓存系统**

**设计理念**：高效管理已编译的着色器模块，避免重复编译。

**核心数据结构**：

```rust
/// 单个着色器的缓存数据
struct ShaderData<ShaderModule> {
    /// 使用该着色器的管线 ID 集合
    pipelines: HashSet<CachedPipelineId>,
    /// 按宏定义分组的已处理着色器
    processed_shaders: HashMap<Box<[ShaderDefVal]>, Arc<ShaderModule>>,
    /// 已解析的导入映射
    resolved_imports: HashMap<ShaderImport, AssetId<Shader>>,
    /// 依赖此着色器的其他着色器
    dependents: HashSet<AssetId<Shader>>,
}

/// 着色器缓存主结构
pub struct ShaderCache<ShaderModule, RenderDevice> {
    /// 所有着色器的缓存数据
    data: HashMap<AssetId<Shader>, ShaderData<ShaderModule>>,
    /// 加载着色器模块的函数（注入依赖）
    load_module: fn(
        &RenderDevice,
        ShaderCacheSource,
        &ValidateShader,
    ) -> Result<ShaderModule, ShaderCacheError>,
    /// 着色器导入路径到资产 ID 的映射
    import_path_shaders: HashMap<ShaderImport, AssetId<Shader>>,
    /// 等待导入的着色器队列
    waiting_on_import: HashMap<ShaderImport, Vec<AssetId<Shader>>>,
    /// 所有已加载的着色器
    shaders: HashMap<AssetId<Shader>, Shader>,
    /// Naga 合成器（处理导入和宏）
    pub composer: naga_oil::compose::Composer,
}
```

**缓存工作流程**：

```rust
impl<ShaderModule, RenderDevice> ShaderCache<ShaderModule, RenderDevice> {
    /// 获取或编译着色器模块
    pub fn get(
        &mut self,
        render_device: &RenderDevice,
        pipeline: CachedPipelineId,
        id: AssetId<Shader>,
        shader_defs: &[ShaderDefVal],
    ) -> Result<Arc<ShaderModule>, ShaderCacheError> {
        // 1. 检查着色器是否已加载
        let shader = self
            .shaders
            .get(&id)
            .ok_or(ShaderCacheError::ShaderNotLoaded(id))?;

        // 2. 获取或创建着色器缓存数据
        let data = self.data.entry(id).or_default();

        // 3. 检查所有导入是否已解析
        let n_asset_imports = shader
            .imports()
            .filter(|import| matches!(import, ShaderImport::AssetPath(_)))
            .count();
        let n_resolved_asset_imports = data
            .resolved_imports
            .keys()
            .filter(|import| matches!(import, ShaderImport::AssetPath(_)))
            .count();
        if n_asset_imports != n_resolved_asset_imports {
            return Err(ShaderCacheError::ShaderImportNotYetAvailable);
        }

        // 4. 记录使用该着色器的管线
        data.pipelines.insert(pipeline);

        // 5. 检查缓存（按宏定义）
        let module = match data.processed_shaders.entry_ref(shader_defs) {
            EntryRef::Occupied(entry) => {
                // 缓存命中
                entry.into_mut()
            }
            EntryRef::Vacant(entry) => {
                // 缓存未命中，处理着色器
                debug!("processing shader {}, with shader defs {:?}", id, shader_defs);
                
                // 5.1 处理导入
                for import in shader.imports() {
                    Self::add_import_to_composer(
                        &mut self.composer,
                        &self.import_path_shaders,
                        &self.shaders,
                        import,
                    )?;
                }

                // 5.2 合并宏定义（着色器内置 + 运行时）
                let shader_defs_merged = shader_defs
                    .iter()
                    .chain(shader.shader_defs.iter())
                    .map(|def| match def.clone() {
                        ShaderDefVal::Bool(k, v) => (k, ShaderDefValue::Bool(v)),
                        ShaderDefVal::Int(k, v) => (k, ShaderDefValue::Int(v)),
                        ShaderDefVal::UInt(k, v) => (k, ShaderDefValue::UInt(v)),
                    })
                    .collect::<HashMap<_, _>>();

                // 5.3 合成着色器
                let naga_module = self.composer.compose(
                    shader.import_path().module_name(),
                    &shader_defs_merged,
                )?;

                // 5.4 编译为 GPU 模块
                let module = (self.load_module)(
                    render_device,
                    ShaderCacheSource::Naga(naga_module),
                    &shader.validate_shader,
                )?;

                // 5.5 缓存结果
                entry.insert(Arc::new(module))
            }
        };

        Ok(module.clone())
    }
    
    /// 添加着色器到缓存
    pub fn set_shader(&mut self, id: AssetId<Shader>, shader: Shader) -> Vec<CachedPipelineId> {
        // 1. 清除旧缓存
        let pipelines_to_queue = self.clear(id);
        
        // 2. 注册导入路径
        let path = shader.import_path();
        self.import_path_shaders.insert(path.clone(), id);
        
        // 3. 解决等待该导入的着色器
        if let Some(waiting_shaders) = self.waiting_on_import.get_mut(path) {
            for waiting_shader in waiting_shaders.drain(..) {
                let data = self.data.entry(waiting_shader).or_default();
                data.resolved_imports.insert(path.clone(), id);
                
                let data = self.data.entry(id).or_default();
                data.dependents.insert(waiting_shader);
            }
        }
        
        // 4. 处理该着色器的导入
        for import in shader.imports() {
            if let Some(import_id) = self.import_path_shaders.get(import).copied() {
                // 导入已存在，直接解析
                let data = self.data.entry(id).or_default();
                data.resolved_imports.insert(import.clone(), import_id);
                
                let data = self.data.entry(import_id).or_default();
                data.dependents.insert(id);
            } else {
                // 导入未加载，加入等待队列
                let waiting = self.waiting_on_import.entry(import.clone()).or_default();
                waiting.push(id);
            }
        }
        
        // 5. 存储着色器
        self.shaders.insert(id, shader);
        
        pipelines_to_queue
    }
    
    /// 清除着色器缓存
    fn clear(&mut self, id: AssetId<Shader>) -> Vec<CachedPipelineId> {
        let mut shaders_to_clear = vec![id];
        let mut pipelines_to_queue = Vec::new();
        
        while let Some(handle) = shaders_to_clear.pop() {
            if let Some(data) = self.data.get_mut(&handle) {
                // 清除已处理的着色器
                data.processed_shaders.clear();
                // 收集需要重新编译的管线
                pipelines_to_queue.extend(data.pipelines.iter().copied());
                // 递归清除依赖此着色器的着色器
                shaders_to_clear.extend(data.dependents.iter().copied());
                
                // 从合成器中移除模块
                if let Some(Shader { import_path, .. }) = self.shaders.get(&handle) {
                    self.composer
                        .remove_composable_module(&import_path.module_name());
                }
            }
        }
        
        pipelines_to_queue
    }
}
```

**缓存优化策略**：

1. **多级缓存**：
   - L1：已处理着色器（按宏定义分组）
   - L2：Naga 模块（合成后的 AST）
   - L3：GPU 模块（编译后的二进制）

2. **依赖跟踪**：
   - 记录着色器之间的依赖关系
   - 当依赖变化时自动失效缓存
   - 递归清除相关缓存

3. **懒加载**：
   - 着色器在首次使用时才编译
   - 导入在需要时才解析
   - 减少启动时间

---

### 5. **验证系统**

**设计理念**：在安全性和性能之间提供可配置的权衡。

**实现机制**：

```rust
/// 描述是否对着色器执行运行时检查
#[derive(Clone, Debug, Default)]
pub enum ValidateShader {
    #[default]
    /// 禁用运行时检查
    ///
    /// 适用于可信的着色器（由程序或信任的依赖编写）
    Disabled,
    
    /// 启用运行时检查（如边界检查）
    ///
    /// 虽然会影响性能，但在加载不可信着色器时应始终启用
    /// 例如：着色器游乐场、用户生成的着色器、Web 浏览器等
    Enabled,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct Shader {
    // ...
    
    /// 启用或禁用运行时着色器验证
    pub validate_shader: ValidateShader,
}
```

**使用场景**：

```rust
// 开发环境 - 启用验证
let debug_shader = Shader {
    validate_shader: ValidateShader::Enabled,
    ..default()
};

// 生产环境 - 禁用验证以提高性能
let release_shader = Shader {
    validate_shader: ValidateShader::Disabled,
    ..default()
};

// 着色器游乐场 - 必须启用验证
let playground_shader = Shader {
    validate_shader: ValidateShader::Enabled,
    ..default()
};
```

**验证内容**：
- ✅ 边界检查：数组访问、循环范围
- ✅ 类型安全：类型转换、函数调用
- ✅ 资源限制：绑定数量、缓冲区大小
- ✅ 语法检查：语法错误、语义错误

---

### 6. **资产加载器**

**设计理念**：与 Bevy 资产系统深度集成，支持异步加载和热重载。

**实现机制**：

```rust
#[derive(Default, TypePath)]
pub struct ShaderLoader;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ShaderLoaderError {
    #[error("Could not load shader: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse shader: {0}")]
    Parse(#[from] alloc::string::FromUtf8Error),
}

/// 着色器加载设置
#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
pub struct ShaderSettings {
    /// 为此着色器指定的 `#define`
    pub shader_defs: Vec<ShaderDefVal>,
}

impl AssetLoader for ShaderLoader {
    type Asset = Shader;
    type Settings = ShaderSettings;
    type Error = ShaderLoaderError;
    
    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Shader, Self::Error> {
        // 1. 获取文件扩展名
        let ext = load_context
            .path()
            .path()
            .extension()
            .unwrap()
            .to_str()
            .unwrap();
        
        // 2. 规范化路径
        let path = load_context.path().to_string();
        let path = path.replace(std::path::MAIN_SEPARATOR, "/");
        
        // 3. 读取文件内容
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        
        // 4. 警告：非 WGSL 着色器不支持宏定义
        if ext != "wgsl" && !settings.shader_defs.is_empty() {
            tracing::warn!(
                "Tried to load a non-wgsl shader with shader defs, this isn't supported: 
                    The shader defs will be ignored."
            );
        }
        
        // 5. 根据扩展名创建着色器
        let mut shader = match ext {
            "spv" => Shader::from_spirv(
                bytes, 
                load_context.path().path().to_string_lossy()
            ),
            "wgsl" => Shader::from_wgsl_with_defs(
                String::from_utf8(bytes)?,
                path,
                settings.shader_defs.clone(),
            ),
            "vert" => Shader::from_glsl(
                String::from_utf8(bytes)?,
                naga::ShaderStage::Vertex,
                path,
            ),
            "frag" => Shader::from_glsl(
                String::from_utf8(bytes)?,
                naga::ShaderStage::Fragment,
                path,
            ),
            "comp" => Shader::from_glsl(
                String::from_utf8(bytes)?,
                naga::ShaderStage::Compute,
                path,
            ),
            #[cfg(feature = "shader_format_wesl")]
            "wesl" => Shader::from_wesl(
                String::from_utf8(bytes)?,
                path,
            ),
            _ => panic!("unhandled extension: {ext}"),
        };
        
        // 6. 收集并存储文件依赖
        for import in &shader.imports {
            if let ShaderImport::AssetPath(asset_path) = import {
                shader.file_dependencies.push(load_context.load(asset_path));
            }
        }
        
        Ok(shader)
    }
    
    fn extensions(&self) -> &[&str] {
        &["spv", "wgsl", "vert", "frag", "comp", "wesl"]
    }
}
```

**热重载支持**：

```rust
// 当着色器文件修改时，资产系统会自动重新加载
// 缓存系统会清除旧缓存并重新编译
app.add_plugins(DefaultPlugins.set(AssetPlugin {
    watch_for_changes: true, // 启用热重载
    ..default()
}));
```

---

### 7. **嵌入着色器宏**

**设计理念**：提供便捷的宏，将着色器嵌入到二进制文件中。

**实现机制**：

```rust
/// 将着色器内联为 `embedded_asset` 并永久加载
///
/// 这解决了着色器加载器无法正确加载着色器依赖的限制
#[macro_export]
macro_rules! load_shader_library {
    ($asset_server_provider: expr, $path: literal $(, $settings: expr)?) => {
        // 1. 嵌入资产
        $crate::_macro::bevy_asset::embedded_asset!($asset_server_provider, $path);
        
        // 2. 加载嵌入的资产
        let handle: $crate::_macro::bevy_asset::prelude::Handle<$crate::prelude::Shader> = 
            $crate::_macro::bevy_asset::load_embedded_asset!(
                $asset_server_provider,
                $path
                $(,$settings)?
            );
        
        // 3. 忘记 handle，防止资产被卸载
        core::mem::forget(handle);
    }
}
```

**使用示例**：

```rust
// 在插件中嵌入着色器
impl Plugin for SpriteRenderPlugin {
    fn build(&self, app: &mut App) {
        // 嵌入并加载精灵着色器库
        load_shader_library!(app, "render/sprite_view_bindings.wgsl");
        
        // 嵌入精灵着色器
        embedded_asset!(app, "render/sprite.wgsl");
        
        // ...
    }
}
```

**优势**：
- ✅ 无需外部文件：着色器直接嵌入二进制
- ✅ 快速加载：避免文件 I/O
- ✅ 依赖管理：自动处理导入
- ✅ 防止卸载：使用 `mem::forget` 保持加载状态

---

### 8. **Naga 集成**

**设计理念**：使用 Naga 作为着色器中间表示，实现多格式转换和优化。

**Naga 工作流程**：

```
用户着色器 (WGSL/GLSL/SPIR-V)
    ↓
Naga 前端解析
    ↓
Naga IR (中间表示)
    ↓
Naga 验证
    ↓
Naga 优化
    ↓
Naga 后端生成
    ↓
GPU 着色器 (WGSL/SPIR-V/HLSL/GLSL)
```

**在 bevy_shader 中的应用**：

```rust
pub struct ShaderCache<ShaderModule, RenderDevice> {
    /// Naga 合成器
    pub composer: naga_oil::compose::Composer,
}

impl<ShaderModule, RenderDevice> ShaderCache<ShaderModule, RenderDevice> {
    pub fn new(
        features: Features,
        downlevel: DownlevelFlags,
        load_module: fn(...),
    ) -> Self {
        // 根据功能设置能力
        let capabilities = get_capabilities(features, downlevel);
        
        // 创建合成器
        #[cfg(debug_assertions)]
        let composer = naga_oil::compose::Composer::default();
        #[cfg(not(debug_assertions))]
        let composer = naga_oil::compose::Composer::non_validating();
        
        let composer = composer.with_capabilities(capabilities);
        
        Self {
            composer,
            // ...
        }
    }
    
    pub fn get(
        &mut self,
        render_device: &RenderDevice,
        pipeline: CachedPipelineId,
        id: AssetId<Shader>,
        shader_defs: &[ShaderDefVal],
    ) -> Result<Arc<ShaderModule>, ShaderCacheError> {
        // ...
        
        // 使用 Naga 合成器合成着色器
        let naga_module = self.composer.compose(
            shader.import_path().module_name(),
            &shader_defs_merged,
        )?;
        
        // 编译为 GPU 模块
        let module = (self.load_module)(
            render_device,
            ShaderCacheSource::Naga(naga_module),
            &shader.validate_shader,
        )?;
        
        Ok(Arc::new(module))
    }
}
```

**Naga 能力映射**：

```rust
fn get_capabilities(features: Features, downlevel: DownlevelFlags) -> Capabilities {
    let mut capabilities = Capabilities::all();
    
    // 根据 GPU 功能设置能力
    capabilities.set(
        Capabilities::SHADER_FLOAT64,
        features.contains(Features::SHADER_F64),
    );
    capabilities.set(
        Capabilities::SHADER_INT64,
        features.contains(Features::SHADER_INT64),
    );
    capabilities.set(
        Capabilities::MULTIVIEW,
        features.contains(Features::MULTIVIEW),
    );
    capabilities.set(
        Capabilities::RAY_QUERY,
        features.contains(Features::EXPERIMENTAL_RAY_QUERY),
    );
    // ... 更多能力
    
    capabilities
}
```

**优势**：
- ✅ 多格式支持：WGSL、GLSL、SPIR-V 互转
- ✅ 验证：语法和语义检查
- ✅ 优化：死代码消除、常量折叠等
- ✅ 跨平台：生成目标平台的最佳格式

---

## 三、典型使用场景

### 场景 1：加载简单着色器

```rust
// 从文件加载
let shader_handle: Handle<Shader> = asset_server.load("shaders/sprite.wgsl");

// 从字符串创建
let shader = Shader::from_wgsl(
    r#"
        @vertex
        fn vertex(in: VertexInput) -> VertexOutput {
            return VertexOutput {
                @builtin(position) clip_position: vec4<f32>(in.position, 0.0, 1.0),
            };
        }
    "#,
    "shaders/vertex.wgsl",
);
```

### 场景 2：带宏定义的着色器

```rust
let shader = Shader::from_wgsl_with_defs(
    r#"
        #if TONEMAP_IN_SHADER
            #define LUT_BINDING 1
        #endif
        
        @fragment
        fn fragment(in: VertexInput) -> @location(0) vec4<f32> {
            // ...
        }
    "#,
    "shaders/pbr.wgsl",
    vec![
        ShaderDefVal::Bool("TONEMAP_IN_SHADER".into(), true),
        ShaderDefVal::UInt("SAMPLE_COUNT".into(), 4),
    ],
);
```

### 场景 3：模块化着色器

```wgsl
// common/constants.wgsl
const PI: f32 = 3.1415926535;
const TWO_PI: f32 = 6.283185307;

// lighting/pbr.wgsl
#import "common/constants.wgsl"

fn calculate_lighting() -> vec3<f32> {
    // 使用导入的常量
    return vec3<f32>(PI);
}

// main.wgsl
#import "lighting/pbr.wgsl"

@fragment
fn fragment(in: VertexInput) -> @location(0) vec4<f32> {
    let color = calculate_lighting();
    return vec4<f32>(color, 1.0);
}
```

### 场景 4：着色器缓存使用

```rust
// 创建缓存
let mut shader_cache = ShaderCache::new(
    render_device.features(),
    render_device.limits().downlevel_flags,
    |device, source, validate| {
        // 自定义加载函数
        device.create_shader_module(&source.into())
    },
);

// 添加着色器
let shader_id = shader_assets.get_id(&shader_handle);
shader_cache.set_shader(shader_id, shader);

// 获取编译后的模块
let module = shader_cache.get(
    &render_device,
    pipeline_id,
    shader_id,
    &[ShaderDefVal::Bool("FEATURE_X".into(), true)],
)?;
```

---

## 四、设计模式总结

### 使用的设计模式

| 模式 | 应用场景 | 示例 |
|------|----------|------|
| **策略模式** | 多格式支持 | `Source` 枚举 |
| **工厂模式** | 着色器创建 | `Shader::from_wgsl` 等 |
| **享元模式** | 着色器缓存 | `processed_shaders` HashMap |
| **观察者模式** | 依赖跟踪 | `dependents` HashSet |
| **模板方法** | 资产加载 | `AssetLoader` trait |
| **建造者模式** | 着色器配置 | `Shader::with_import_path` |
| **单例模式** | 嵌入着色器 | `load_shader_library!` 宏 |

---

## 五、性能优化策略

### 1. **缓存优化**
- ✅ 按宏定义缓存：不同宏定义产生不同缓存条目
- ✅ 懒加载：着色器在首次使用时才编译
- ✅ 依赖跟踪：智能失效相关缓存

### 2. **编译优化**
- ✅ Naga 优化：死代码消除、常量折叠
- ✅ 并行编译：支持多线程编译（如果启用）
- ✅ 预编译：发布时预编译为 SPIR-V

### 3. **加载优化**
- ✅ 嵌入资产：避免文件 I/O
- ✅ 异步加载：不阻塞主线程
- ✅ 热重载：开发时快速迭代

### 4. **验证优化**
- ✅ 可配置验证：开发时启用，发布时禁用
- ✅ 增量验证：只验证修改的部分
- ✅ 缓存验证结果：避免重复验证

---

## 六、与其他模块的协作

### 与 bevy_render

```rust
// bevy_render 使用 bevy_shader 加载和缓存着色器
pub struct PipelineCache {
    shader_cache: ShaderCache<GpuShaderModule, RenderDevice>,
}

impl PipelineCache {
    pub fn queue_render_pipeline(&mut self, descriptor: RenderPipelineDescriptor) {
        // 使用 bevy_shader 编译着色器
        let shader_module = self.shader_cache.get(
            &self.render_device,
            pipeline_id,
            shader_id,
            &descriptor.shader_defs,
        )?;
        
        // 创建渲染管线
        let pipeline = self.render_device.create_render_pipeline(&descriptor);
    }
}
```

### 与 bevy_asset

```rust
// bevy_asset 使用 bevy_shader 的 AssetLoader
app.init_asset_loader::<ShaderLoader>();

// 加载着色器
let handle: Handle<Shader> = asset_server.load("shaders/sprite.wgsl");

// 热重载
asset_server.watch_for_changes().unwrap();
```

### 与 bevy_material

```rust
// bevy_material 使用 bevy_shader 管理材质着色器
#[derive(Asset, TypePath, AsBindGroup)]
pub struct ColorMaterial {
    #[uniform(0)]
    pub color: Color,
}

impl Material for ColorMaterial {
    fn fragment_shader() -> ShaderRef {
        // 返回着色器引用
        "shaders/color_material.wgsl".into()
    }
}
```

---

## 七、设计优势与权衡

### 优势

1. **灵活性**：支持多种着色器格式和宏定义
2. **性能**：高效的缓存和懒加载策略
3. **可维护性**：模块化设计，清晰的职责划分
4. **安全性**：可配置的验证系统
5. **开发体验**：热重载、嵌入资产、错误提示

### 权衡

1. **复杂度**：缓存和依赖管理增加复杂度
2. **编译时间**：首次使用时的编译延迟
3. **内存占用**：缓存多个变体占用更多内存
4. **学习曲线**：理解宏定义和导入系统需要时间

---

## 八、总结

`bevy_shader` 库体现了现代游戏引擎着色器管理的最佳实践：

- ✅ **多格式支持**：WGSL、GLSL、SPIR-V、Wesl
- ✅ **模块化开发**：`#import` 预处理指令
- ✅ **参数化着色器**：运行时宏定义
- ✅ **高效缓存**：多级缓存和依赖跟踪
- ✅ **安全验证**：可配置的运行时检查
- ✅ **无缝集成**：与 Bevy 资产系统深度集成
- ✅ **Naga 优化**：多格式转换和优化

这种设计使得 `bevy_shader` 既适合简单的着色器需求，也能满足复杂游戏的高级特性，是 Bevy 引擎渲染系统的核心基础设施。

---

**文档版本**：Bevy Engine 0.19.0-dev  
**最后更新**：2026-01-21  
**分析范围**：crates/bevy_shader 源代码
