pub extern crate libdogd;
pub extern crate paste;
pub extern crate bincode;
pub extern crate serde;

#[macro_export]
macro_rules! decl_service {
    ($name: ident, $( $fn:ident ($($arg:ident: $ty:ty),*) $ret:ty ),*) => {
        ::demonite::paste::paste! {
            #[derive(::demonite::serde::Serialize, ::demonite::serde::Deserialize, Debug)]
            #[serde(crate = "::demonite::serde")]
            #[allow(non_camel_case_types)]
            pub enum $name {
                $( [<_ $fn>]($($ty),*)),*
            }
            impl $name {
                pub(crate) fn run(self) -> Vec<u8> {
                    match self {
                        $($name::[<_ $fn>]($($arg),*) => 
                          ::demonite::bincode::serialize(& $fn( $($arg),* ) ).unwrap()),*
                    }
                }
                pub fn path() -> std::path::PathBuf {
                    let mut path = std::path::PathBuf::from(std::env::var("XDG_RUNTIME_DIR")
                        .expect("Failed to get XDG_RUNTIME_DIR"));
                    path.push("demonite");
                    path.push(stringify!($name));
                    path
                }
                pub(crate) fn launch() {
                    use std::{
                        env,
                        fs,
                        path::PathBuf,
                        io::Write,
                        os::unix::net::{UnixStream, UnixListener},
                    };
                    use ::demonite::bincode;

                    if !Self::path().parent().unwrap().exists() {
                        fs::create_dir(Self::path().parent().unwrap())
                            .expect("Failed to ensure that XDG_RUNTIME_DIR/demonite/ dir exists");
                    }
                    if Self::path().exists() {
                        fs::remove_file(Self::path())
                            .expect("Failed to ensure that socket file does not exist");
                    }
                    let sock = UnixListener::bind(Self::path()).expect("Failed to listen");
                    ::demonite::libdogd::log_info(format!("Listening on {:?}", sock.local_addr()));
                    for stream in sock.incoming() {
                        if let Ok(mut s) = stream {
                            let variant = match bincode::deserialize_from::<&UnixStream, Self>(&s) {
                                Ok(v) => v,
                                Err(e) => {
                                    ::demonite::libdogd::log_error(format!("Failed deserializing packet: {}", e));
                                    continue;
                                },
                            };
                            ::demonite::libdogd::log_debug(format!("Method call: {:?}", &variant));
                            let response = variant.run();
                            match s.write_all(&response) {
                                Ok(_) => (),
                                Err(e) => {
                                    ::demonite::libdogd::log_error(format!("Failed to write response: {}", e));
                                },
                            }
                        }
                    }
                }
                $(
                    pub fn $fn($($arg: $ty),*) -> $ret {
                        use ::demonite::bincode;
                        use ::std::os::unix::net::UnixStream;
                        use ::std::io::Write;
                        let packet: Vec<u8> = bincode::serialize(&Self::[<_ $fn>]($($arg),*)).unwrap();
                        let mut stream = UnixStream::connect(Self::path()).unwrap();
                        stream.write_all(&packet);
                        bincode::deserialize_from(&stream).unwrap()
                    }
                 )*
            }
        }
    }
}
