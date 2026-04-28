use mpv_client::{Event, Handle, mpv_handle};

#[unsafe(no_mangle)]
extern "C" fn mpv_open_cplugin(handle: *mut mpv_handle) -> std::os::raw::c_int {
    let client = Handle::from_ptr(handle);

    println!("Hello world from Rust plugin {}!", client.name());

    loop {
        match client.wait_event(-1.) {
            Event::Shutdown => {
                return 0;
            }
            event => {
                println!("Got event: {event}");
            }
        }
    }
}
