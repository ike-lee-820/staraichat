package cc.ccwu.staraichat;

import android.app.Activity;
import android.app.NativeActivity;
import android.content.ContentResolver;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

public class StarAIChatActivity extends NativeActivity {
    private static final int REQUEST_OPEN_DOCUMENT = 1001;
    private static final int REQUEST_CREATE_DOCUMENT = 1002;

    private static StarAIChatActivity sInstance;
    private volatile String mPickResult;
    private volatile String mSaveResult;
    private volatile boolean mPickDone;
    private volatile boolean mSaveDone;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        sInstance = this;
    }

    public static StarAIChatActivity getInstance() {
        return sInstance;
    }

    public void startPickFile() {
        mPickDone = false;
        mPickResult = null;
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("*/*");
        startActivityForResult(intent, REQUEST_OPEN_DOCUMENT);
    }

    public void startSaveFile(String defaultName) {
        mSaveDone = false;
        mSaveResult = null;
        Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("application/octet-stream");
        intent.putExtra(Intent.EXTRA_TITLE, defaultName);
        startActivityForResult(intent, REQUEST_CREATE_DOCUMENT);
    }

    public boolean isPickDone() {
        return mPickDone;
    }

    public String getPickResult() {
        return mPickResult;
    }

    public boolean isSaveDone() {
        return mSaveDone;
    }

    public String getSaveResult() {
        return mSaveResult;
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (resultCode != Activity.RESULT_OK || data == null) {
            if (requestCode == REQUEST_OPEN_DOCUMENT) {
                mPickDone = true;
            } else if (requestCode == REQUEST_CREATE_DOCUMENT) {
                mSaveDone = true;
            }
            return;
        }
        Uri uri = data.getData();
        if (uri == null) {
            if (requestCode == REQUEST_OPEN_DOCUMENT) {
                mPickDone = true;
            } else if (requestCode == REQUEST_CREATE_DOCUMENT) {
                mSaveDone = true;
            }
            return;
        }
        if (requestCode == REQUEST_OPEN_DOCUMENT) {
            mPickResult = copyUriToCache(uri) + "\n" + queryDisplayName(uri);
            mPickDone = true;
        } else if (requestCode == REQUEST_CREATE_DOCUMENT) {
            mSaveResult = uri.toString();
            mSaveDone = true;
        }
    }

    private String copyUriToCache(Uri uri) {
        try {
            ContentResolver resolver = getContentResolver();
            InputStream in = resolver.openInputStream(uri);
            if (in == null) return "";
            File cacheDir = getCacheDir();
            String displayName = queryDisplayName(uri);
            String ext = "";
            int dot = displayName.lastIndexOf('.');
            if (dot > 0) ext = displayName.substring(dot);
            String random = Long.toHexString(System.currentTimeMillis());
            File outFile = new File(cacheDir, "picker_" + random + ext);
            OutputStream out = new FileOutputStream(outFile);
            byte[] buf = new byte[8192];
            int n;
            while ((n = in.read(buf)) > 0) {
                out.write(buf, 0, n);
            }
            in.close();
            out.close();
            return outFile.getAbsolutePath();
        } catch (Exception e) {
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
