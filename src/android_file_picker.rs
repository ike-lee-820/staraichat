use anyhow::{Context, Result};
use jni::objects::JString;
use jni::signature::JavaType;
use jni::JavaVM;
use std::sync::{Arc, Mutex, OnceLock};

static APP_VM: OnceLock<*mut jni::sys::JavaVM> = OnceLock::new();
static PICK_RESULT: OnceLock<Arc<Mutex<Option<(String, String)>>>> = OnceLock::new();
static SAVE_RESULT: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

pub fn set_vm(vm: *mut jni::sys::JavaVM) {
    let _ = APP_VM.set(vm);
    let _ = PICK_RESULT.set(Arc::new(Mutex::new(None)));
    let _ = SAVE_RESULT.set(Arc::new(Mutex::new(None)));
}

fn attach_env() -> Result<jni::AttachGuard<'static>> {
    let vm_ptr = APP_VM.get().context("Android VM not set")?;
    let vm = unsafe { JavaVM::from_raw(*vm_ptr) }?;
    Ok(vm.attach_current_thread()?)
}

fn with_env<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut jni::JNIEnv) -> Result<T>,
{
    let mut env = attach_env()?;
    f(&mut env)
}

pub fn pick_file() -> Result<Option<(String, String)>> {
    with_env(|env| {
        let cls = env.find_class("cc/ccwu/staraichat/StarAIChatActivity")?;
        let activity = env
            .call_static_method(
                &cls,
                "getInstance",
                "()Lcc/ccwu/staraichat/StarAIChatActivity;",
                &[],
            )?
            .l()?;

        env.call_method(
            &activity,
            "startPickFile",
            "()V",
            &[],
        )?;

        for _ in 0..300 {
            let done = env
                .call_method(
                    &activity,
                    "isPickDone",
                    "()Z",
                    &[],
                )?
                .z()?;
            if done {
                let result: JString = env
                    .call_method(
                        &activity,
                        "getPickResult",
                        "()Ljava/lang/String;",
                        &[],
                    )?
                    .l()?
                    .into();
                let s: String = env.get_string(&result)?.into();
                if s.is_empty() {
                    return Ok(None);
                }
                let mut parts = s.splitn(2, '\n');
                let path = parts.next().unwrap_or("").to_string();
                let name = parts.next().unwrap_or("").to_string();
                return Ok(Some((name, path)));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        anyhow::bail!("pick file timeout")
    })
}

pub fn save_file(default_name: &str) -> Result<Option<String>> {
    with_env(|env| {
        let cls = env.find_class("cc/ccwu/staraichat/StarAIChatActivity")?;
        let activity = env
            .call_static_method(
                &cls,
                "getInstance",
                "()Lcc/ccwu/staraichat/StarAIChatActivity;",
                &[],
            )?
            .l()?;

        env.call_method(
            &activity,
            "startSaveFile",
            "(Ljava/lang/String;)V",
            &[(&env.new_string(default_name)?).into()],
        )?;

        for _ in 0..300 {
            let done = env
                .call_method(
                    &activity,
                    "isSaveDone",
                    "()Z",
                    &[],
                )?
                .z()?;
            if done {
                let result: JString = env
                    .call_method(
                        &activity,
                        "getSaveResult",
                        "()Ljava/lang/String;",
                        &[],
                    )?
                    .l()?
                    .into();
                let s: String = env.get_string(&result)?.into();
                if s.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(s));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        anyhow::bail!("save file timeout")
    })
}

pub fn write_uri(uri: &str, data: &[u8]) -> Result<()> {
    with_env(|env| {
        let cls = env.find_class("cc/ccwu/staraichat/StarAIChatActivity")?;
        let activity = env
            .call_static_method(
                &cls,
                "getInstance",
                "()Lcc/ccwu/staraichat/StarAIChatActivity;",
                &[],
            )?
            .l()?;

        let resolver = env
            .call_method(
                &activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )?
            .l()?;

        let uri_cls = env.find_class("android/net/Uri")?;
        let uri_obj = env
            .call_static_method(
                &uri_cls,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[(&env.new_string(uri)?).into()],
            )?
            .l()?;

        let output = env
            .call_method(
                &resolver,
                "openOutputStream",
                "(Landroid/net/Uri;)Ljava/io/OutputStream;",
                &[(&uri_obj).into()],
            )?
            .l()?;

        let chunk_size = 8192;
        let mut offset = 0;
        while offset < data.len() {
            let end = (offset + chunk_size).min(data.len());
            let chunk = &data[offset..end];
            let arr = env.byte_array_from_slice(chunk)?;
            env.call_method(
                &output,
                "write",
                "([B)V",
                &[(&arr).into()],
            )?;
            offset = end;
        }

        env.call_method(&output, "close", "()V", &[])?;
        Ok(())
    })
}
