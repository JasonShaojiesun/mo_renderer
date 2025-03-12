use super::App;
use downcast_rs::{impl_downcast, Downcast};

use std::any::Any;

/// Plugins state in the application
#[derive(PartialEq, Eq, Debug, Clone, Copy, PartialOrd, Ord)]
pub enum PluginState {
    /// Plugins are being added.
    Adding,
    /// All plugins already added are ready.
    Ready,
    /// Finish has been executed for all plugins added.
    Finished,
    /// Cleanup has been executed for all plugins added.
    Cleaned,
}

/// 插件系统的根 Trait，所有希望注册为插件的类必须实现这个 Trait。
///
/// 插件通常用于向 App 注册 [`Resource`]、[`Event`]、[`System`] 等。
pub trait PluginTrait: Downcast + Any + Send + Sync {
    /// Configures the [`App`] to which this plugin is added.
    fn build(&self, app: &mut App);

    /// 插件是否已经完成了它的设置？主要用于需要异步操作的插件，比如渲染器的初始化。
    /// 一旦插件准备好，就应该调用 [`finish`](PluginTrait::finish)。
    fn ready(&self, _app: &App) -> bool {
        true
    }

    /// 一旦所有已注册的插件都准备好，完成将此插件添加到 [`App`]。主要用于需要异步操作的插件，比如渲染器的初始化。
    fn finish(&self, _app: &mut App) {
        // do nothing
    }

    /// Runs after all plugins are built and finished, but before the app schedule is executed.
    /// This can be useful if you have some resource that other plugins need during their build step,
    /// but after build you want to remove it and send it to another thread.
    fn cleanup(&self, _app: &mut App) {
        // do nothing
    }

    /// Configures a name for the [`PluginTrait`] which is primarily used for checking plugin
    /// uniqueness and debugging.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    /// If the plugin can be meaningfully instantiated several times in an [`App`],
    /// override this method to return `false`.
    fn is_unique(&self) -> bool {
        true
    }
}

impl_downcast!(PluginTrait);

/// 给所有接受 [`&mut App`] 的闭包函数实现了 [`PluginTrait`] Trait。
impl<T: Fn(&mut App) + Send + Sync + 'static> PluginTrait for T {
    fn build(&self, app: &mut App) {
        self(app);
    }
}
