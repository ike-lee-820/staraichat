use anyhow::{Context, Result};
use jni::objects::JString;
use jni::JavaVM;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

static APP_VM: OnceLock<usize> = OnceLock::new();

pub fn set_vm(vm: *mut jni::sys::JavaVM) {
    let _ = APP_VM.set(vm as usize);
}

fn get_vm() -> Result<JavaVM> {
    let ptr = APP_VM.get().context("Android VM not set")?;
    unsafe { Ok(JavaVM::from_raw(*ptr as *mut jni::sys::JavaVM)?) }
}

fn with_env<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut jni::JNIEnv) -> Result<T>,
{
    let vm = get_vm()?;
    let mut env = vm.attach_current_thread()?;
    f(&mut env)
}

fn start_pick_file() -> Result<()> {
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
        env.call_method(&activity, "startPickFile", "()V", &[])?;
        Ok(())
    })
}

fn start_save_file(default_name: &str) -> Result<()> {
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
        Ok(())
    })
}

fn is_pick_done() -> Result<bool> {
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
        let done = env.call_method(&activity, "isPickDone", "()Z", &[])?.z()?;
        Ok(done)
    })
}

fn get_pick_result() -> Result<Option<(String, String)>> {
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
        let result: JString = env
            .call_method(&activity, "getPickResult", "()Ljava/lang/String;", &[])?
            .l()?
            .into();
        let s: String = env.get_string(&result)?.into();
        if s.is_empty() {
            return Ok(None);
        }
        let mut parts = s.splitn(2, '\n');
        let path = parts.next().unwrap_or("").to_string();
        let name = parts.next().unwrap_or("").to_string();
        Ok(Some((name, path)))
    })
}

fn is_save_done() -> Result<bool> {
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
        let done = env.call_method(&activity, "isSaveDone", "()Z", &[])?.z()?;
        Ok(done)
    })
}

fn get_save_result() -> Result<Option<String>> {
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
        let result: JString = env
            .call_method(&activity, "getSaveResult", "()Ljava/lang/String;", &[])?
            .l()?
            .into();
        let s: String = env.get_string(&result)?.into();
        Ok(if s.is_empty() { None } else { Some(s) })
    })
}

pub fn pick_file() -> Result<Option<(String, String)>> {
    start_pick_file()?;
    let result = thread::spawn(|| {
        for _ in 0..300 {
            match is_pick_done() {
                Ok(true) => return get_pick_result(),
                Ok(false) => thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(e),
            }
        }
        anyhow::bail!("pick file timeout")
    })
    .join()
    .map_err(|_| anyhow::anyhow!("pick file thread panicked"))?;
    result
}

pub fn save_file(default_name: &str) -> Result<Option<String>> {
    start_save_file(default_name)?;
    let result = thread::spawn(|| {
        for _ in 0..300 {
            match is_save_done() {
                Ok(true) => return get_save_result(),
                Ok(false) => thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(e),
            }
        }
        anyhow::bail!("save file timeout")
    })
    .join()
    .map_err(|_| anyhow::anyhow!("save file thread panicked"))?;
    result
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
            env.call_method(&output, "write", "([B)V", &[(&arr).into()])?;
            offset = end;
        }

        env.call_method(&output, "close", "()V", &[])?;
        Ok(())
    })
}
