package cc.ccwu.staraichat;

import android.app.Activity;
import android.app.NativeActivity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.ref.WeakReference;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;

public class StarAIChatActivity extends NativeActivity {
    private static final int REQUEST_OPEN_DOCUMENT = 1001;
    private static final int REQUEST_CREATE_DOCUMENT = 1002;

    private static WeakReference<StarAIChatActivity> sInstanceRef = new WeakReference<>(null);

    private final AtomicReference<String> mPickResult = new AtomicReference<>(null);
    private final AtomicReference<String> mSaveResult = new AtomicReference<>(null);
    private final AtomicBoolean mPickDone = new AtomicBoolean(false);
    private final AtomicBoolean mSaveDone = new AtomicBoolean(false);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        sInstanceRef = new WeakReference<>(this);
    }

    public static StarAIChatActivity getInstance() {
        return sInstanceRef.get();
    }

    public void startPickFile() {
        mPickResult.set(null);
        mPickDone.set(false);
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("*/*");
        startActivityForResult(intent, REQUEST_OPEN_DOCUMENT);
    }

    public void startSaveFile(String defaultName) {
        mSaveResult.set(null);
        mSaveDone.set(false);
        Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("application/octet-stream");
        intent.putExtra(Intent.EXTRA_TITLE, defaultName);
        startActivityForResult(intent, REQUEST_CREATE_DOCUMENT);
    }

    public boolean isPickDone() {
        return mPickDone.get();
    }

    public String getPickResult() {
        return mPickResult.get();
    }

    public boolean isSaveDone() {
        return mSaveDone.get();
    }

    public String getSaveResult() {
        return mSaveResult.get();
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == REQUEST_OPEN_DOCUMENT) {
            handlePickResult(resultCode, data);
        } else if (requestCode == REQUEST_CREATE_DOCUMENT) {
            handleSaveResult(resultCode, data);
        }
    }

    private void handlePickResult(int resultCode, Intent data) {
        if (resultCode != Activity.RESULT_OK || data == null) {
            mPickResult.set("");
            mPickDone.set(true);
            return;
        }
        Uri uri = data.getData();
        if (uri == null) {
            mPickResult.set("");
            mPickDone.set(true);
            return;
        }
        new Thread(() -> {
            String path = copyUriToCache(uri);
            String name = queryDisplayName(uri);
            mPickResult.set(path + "\n" + name);
            mPickDone.set(true);
        }).start();
    }

    private void handleSaveResult(int resultCode, Intent data) {
        if (resultCode != Activity.RESULT_OK || data == null) {
            mSaveResult.set("");
            mSaveDone.set(true);
            return;
        }
        Uri uri = data.getData();
        if (uri == null) {
            mSaveResult.set("");
            mSaveDone.set(true);
            return;
        }
        mSaveResult.set(uri.toString());
        mSaveDone.set(true);
    }

    private String copyUriToCache(Uri uri) {
        File outFile = null;
        try {
            File cacheDir = getCacheDir();
            String displayName = queryDisplayName(uri);
            String ext = "";
            int dot = displayName.lastIndexOf('.');
            if (dot > 0) ext = displayName.substring(dot);
            String random = Long.toHexString(System.currentTimeMillis());
            outFile = new File(cacheDir, "picker_" + random + ext);

            try (InputStream in = getContentResolver().openInputStream(uri);
                 OutputStream out = new FileOutputStream(outFile)) {
                if (in == null) return "";
                byte[] buf = new byte[8192];
                int n;
                while ((n = in.read(buf)) > 0) {
                    out.write(buf, 0, n);
                }
            }
            return outFile.getAbsolutePath();
        } catch (Exception e) {
            if (outFile != null && outFile.exists()) {
                outFile.delete();
            }
            return "";
        }
    }

    private String queryDisplayName(Uri uri) {
        String result = "file";
        try (Cursor cursor = getContentResolver().query(uri, null, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                int idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                if (idx >= 0) result = cursor.getString(idx);
            }
        } catch (Exception e) {
        }
        return result;
    }
}
