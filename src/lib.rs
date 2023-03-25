pub extern crate libdogd;
pub extern crate paste;
pub extern crate bincode;
pub extern crate serde;
pub extern crate thiserror;

#[derive(serde::Serialize, serde::Deserialize, Debug, thiserror::Error)]
pub enum DemoniteErr {
    #[error("Bincode error: {0}")]
    Serialize(String),
    #[error("Input/Output error: {0}")]
    Io(String),
    #[error("Environment Variable Error: {0}")]
    EnvVar(String),
}

// Eh... those errors don't have Serialize implemented.
impl From<bincode::Error> for DemoniteErr {
    fn from(other: bincode::Error) -> Self {
        Self::Serialize(other.to_string())
    }
}

impl From<std::io::Error> for DemoniteErr {
    fn from(other: std::io::Error) -> Self {
        Self::Io(other.to_string())
    }
}

impl From<std::env::VarError> for DemoniteErr {
    fn from(other: std::env::VarError) -> Self {
        Self::EnvVar(other.to_string())
    }
}

#[macro_export]
macro_rules! decl_service {
    ($name: ident, $( $fn:ident ($($arg:ident: $ty:ty),*) $ret:ty ),*) => {
        ::demonite::paste::paste! {
            #[derive(::demonite::serde::Serialize, ::demonite::serde::Deserialize, Debug)]
            #[serde(crate = "::demonite::serde")]
            pub enum $name {
                $( [<_ $fn>]($($ty),*)),*
            }

            impl $name {
                pub(crate) fn run(self) -> Result<Vec<u8>, demonite::DemoniteErr> {
                    match self {
                        $($name::[<_ $fn>]($($arg),*) => 
                          Ok(::demonite::bincode::serialize(& $fn( $($arg),* ) )?)),*
                    }
                }
                pub fn path() -> Result<std::path::PathBuf, demonite::DemoniteErr> {
                    let mut path = std::path::PathBuf::from(std::env::var("XDG_RUNTIME_DIR")?);
                    path.push("demonite");
                    path.push(stringify!($name));
                    Ok(path)
                }
                pub(crate) fn launch() -> Result<(), demonite::DemoniteErr> {
                    use std::{
                        env,
                        fs,
                        path::PathBuf,
                        io::Write,
                        os::unix::net::{UnixStream, UnixListener},
                    };
                    use ::demonite::bincode;

                    if !Self::path()?.parent().unwrap().exists() {
                        fs::create_dir(Self::path()?.parent().unwrap())?;
                    }
                    if Self::path()?.exists() {
                        fs::remove_file(Self::path()?)?;
                    }
                    let sock = UnixListener::bind(Self::path()?)?;
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
                            let response = match variant.run() {
                                Ok(r) => r,
                                Err(e) => {
                                    ::demonite::libdogd::log_error(format!("Failed serializing response: {}", e));
                                    bincode::serialize(&demonite::DemoniteErr::Serialize(e.to_string()))
                                        .unwrap() // is it really going to never fail
                                },
                            };
                            match s.write_all(&response) {
                                Ok(_) => (),
                                Err(e) => {
                                    ::demonite::libdogd::log_error(format!("Failed to write response: {}", e));
                                },
                            }
                        }
                    }
                    Ok(())
                }
                $(
                    pub fn $fn($($arg: $ty),*) -> Result<$ret, demonite::DemoniteErr> {
                        use ::demonite::bincode;
                        use ::std::os::unix::net::UnixStream;
                        use ::std::io::Write;
                        let packet: Vec<u8> = bincode::serialize(&Self::[<_ $fn>]($($arg),*))?;
                        let mut stream = UnixStream::connect(Self::path()?)?;
                        stream.write_all(&packet)?;
                        Ok(bincode::deserialize_from(&stream)?)
                    }
                 )*
            }
        }
    }
}
