//! Functions, types and macros for working with the Java Virtual Machine.
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Once;

use jnix::jni::objects::JObject;
use jnix::jni::JNIEnv;
use jnix::{FromJava, JnixEnv};

use talpid_types::android::AndroidContext;

use crate::mullvad::api;

mod classes;

static LOAD_CLASSES: Once = Once::new();

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to create global reference to Java object")]
    CreateGlobalReference(#[source] jnix::jni::errors::Error),

    #[error("Failed to get Java VM instance")]
    GetJvmInstance(#[source] jnix::jni::errors::Error),
}

pub struct Jvm<'a>(JnixEnv<'a>);

impl<'a> Jvm<'a> {
    pub fn new(env: JNIEnv<'a>) -> Jvm<'a> {
        let env = JnixEnv::from(env);

        LOAD_CLASSES.call_once(|| env.preload_classes(classes::CLASSES.iter().cloned()));

        Self(env)
    }

    pub fn create_android_context(
        &self,
        vpn_service: JObject<'_>,
    ) -> Result<AndroidContext, Error> {
        Ok(AndroidContext {
            jvm: Arc::new(self.0.get_java_vm().map_err(Error::GetJvmInstance)?),
            vpn_service: self
                .0
                .new_global_ref(vpn_service)
                .map_err(Error::CreateGlobalReference)?,
        })
    }

    pub fn pathbuf_from_java(&self, path: JObject<'_>) -> PathBuf {
        PathBuf::from(String::from_java(&self.0, path))
    }

    pub fn api_endpoint_from_java(
        &self,
        endpoint_override: JObject<'_>,
    ) -> Option<mullvad_api::ApiEndpoint> {
        api::api_endpoint_from_java(&self.0, endpoint_override)
    }
}

/// Throw a Java exception and return if `result` is an error
macro_rules! ok_or_throw {
    ($env:expr, $result:expr) => {{
        match $result {
            Ok(val) => val,
            Err(err) => {
                let env = $env;
                env.throw(err.to_string())
                    .expect("Failed to throw exception");
                return;
            }
        }
    }};
}

pub(crate) use ok_or_throw;
