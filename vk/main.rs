use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    Entry, Instance,
    vk::*,
    util::*,
};

fn main() {
    cmn::common::run_sample::<App, Backend>();
}

pub struct Backend {
    instance: Instance,
}

impl cmn::common::Backend for Backend {
    type FrameCtx = ();

    fn initalize(platform: &cmn::common::Platform) -> anyhow::Result<Self> {
        use ash::{vk, Entry};
        let entry = Entry::linked();

        let app_info = vk::ApplicationInfo {
            api_version: vk::make_api_version(0, 1, 0, 0),
            ..Default::default()
            // p_application_name: todo!(),
            // application_version: todo!(),
            // p_engine_name: todo!(),
            // engine_version: todo!(),
        };


        let layer_properties = unsafe { entry.enumerate_instance_layer_properties().expect("can query layers properties") };
        let extensions = unsafe { entry.enumerate_instance_extension_properties(None).expect("can query implementation extensions") };

        let create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info,
            enabled_layer_count: 1,
            pp_enabled_layer_names: todo!(),
            enabled_extension_count: todo!(),
            pp_enabled_extension_names: todo!(),
            _marker: std::marker::PhantomData,
        };
        let instance = unsafe { entry.create_instance(&create_info, None)? };
    }

    fn pre(&mut self) -> Self::FrameCtx {
        ()
    }

    fn post(&mut self) {
        ()
    }
}

pub struct App {

}