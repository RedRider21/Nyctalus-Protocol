// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
package org.nyctalus.mobile;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.os.Build;
import android.os.Environment;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.Map;

/**
 * Nyctalus DENTRO l'app, senza Termux: il motore viaggia nell'APK come libnyctalus.so (Android 10+
 * esegue solo ciò che sta nella cartella delle librerie dell'app) e qui si lancia
 * «libnyctalus.so app», l'orchestratore in Rust: serve la web-app su 127.0.0.1:18731 e avvia
 * chat, rete e servizio .nyct. Tutti i dati stanno nella cartella PRIVATA dell'app
 * (HOME = getFilesDir()): nessun'altra app li legge.
 *
 * Il processo vive finché vive l'app; il servizio in primo piano delle notifiche la tiene in vita
 * anche a schermo spento. Se Android lo ferma, alla riapertura (o dal servizio) si riaccende.
 */
final class MotoreNativo {
    private static Process processo;

    private MotoreNativo() { }

    static File libreria(Context c) {
        return new File(c.getApplicationInfo().nativeLibraryDir, "libnyctalus.so");
    }

    /** L'APK porta il motore? (APK «con tutto»; quelli vecchi lo avviavano in Termux) */
    static boolean disponibile(Context c) {
        File f = libreria(c);
        return f.isFile() && f.canExecute();
    }

    static synchronized boolean vivo() {
        if (processo == null) return false;
        try {
            processo.exitValue();
            return false;
        } catch (IllegalThreadStateException ancoraVivo) {
            return true;
        }
    }

    /** Accende Nyctalus (se non lo è già) con il segreto d'accesso di questa app. */
    static synchronized void avvia(Context ctx, String segreto) {
        Context c = ctx.getApplicationContext();
        File home = c.getFilesDir();
        File dati = new File(home, ".nyctalus-mobile");
        try {
            dati.mkdirs();
            scriviPrivato(new File(dati, "token"), segreto);
            if (vivo()) return;                       // già acceso: rilegge il segreto da solo
            File webapp = preparaWebapp(c);
            File lib = libreria(c);
            ProcessBuilder pb = new ProcessBuilder(lib.getAbsolutePath(), "app", "--webapp", webapp.getAbsolutePath());
            Map<String, String> env = pb.environment();
            env.put("HOME", home.getAbsolutePath());
            env.put("TMPDIR", c.getCacheDir().getAbsolutePath());          // /tmp non c'è su Android
            env.put("XDG_RUNTIME_DIR", c.getCacheDir().getAbsolutePath()); // vista in chiaro della cassaforte
            env.put("NYCTALUS_BIN", lib.getAbsolutePath());
            env.put("NYCTALUS_ANDROID", "1");
            // le cartelle condivise del telefono: galleria (foto/video recenti) e «sfoglia» (sola lettura)
            File mem = Environment.getExternalStorageDirectory();
            env.put("NYCTALUS_GALLERIA", new File(mem, "DCIM") + ":" + new File(mem, "Pictures") + ":" + new File(mem, "Movies"));
            env.put("NYCTALUS_SFOGLIA", new File(mem, "Download") + ":" + new File(mem, "Documents") + ":"
                    + new File(mem, "DCIM") + ":" + new File(mem, "Pictures") + ":" + new File(mem, "Music"));
            pb.directory(home);
            pb.redirectErrorStream(true);
            File registro = new File(dati, "app.log");
            if (registro.length() > 2 * 1024 * 1024) registro.delete();
            pb.redirectOutput(ProcessBuilder.Redirect.appendTo(registro));
            processo = pb.start();
        } catch (Exception e) {
            processo = null;
            try {
                OutputStream o = new FileOutputStream(new File(dati, "app.log"), true);
                o.write(("errore avvio: " + e + "\n").getBytes("UTF-8"));
                o.close();
            } catch (Exception ignorata) { }
        }
    }

    /** Spegne Nyctalus (SIGTERM: l'orchestratore spegne a sua volta chat, rete e servizio .nyct). */
    static synchronized void ferma() {
        if (processo != null) {
            processo.destroy();
            processo = null;
        }
    }

    /** Le ultime righe del registro (per mostrare un errore comprensibile). */
    static String ultimeRighe(Context c, int quante) {
        File f = new File(new File(c.getFilesDir(), ".nyctalus-mobile"), "app.log");
        try (InputStream in = new java.io.FileInputStream(f)) {
            byte[] b = new byte[(int) Math.min(f.length(), 16384)];
            long salta = f.length() - b.length;
            if (salta > 0) in.skip(salta);
            int n = in.read(b);
            String[] r = new String(b, 0, Math.max(0, n), "UTF-8").split("\n");
            StringBuilder s = new StringBuilder();
            for (int i = Math.max(0, r.length - quante); i < r.length; i++) s.append(r[i]).append('\n');
            return s.toString().trim();
        } catch (Exception e) {
            return "";
        }
    }

    /** La web-app esce dall'APK nella cartella privata, solo quando l'app è cambiata. */
    private static File preparaWebapp(Context c) throws Exception {
        File dest = new File(c.getFilesDir(), "webapp");
        PackageInfo pi = c.getPackageManager().getPackageInfo(c.getPackageName(), 0);
        long versione = Build.VERSION.SDK_INT >= 28 ? pi.getLongVersionCode() : pi.versionCode;
        String firma = versione + ":" + pi.lastUpdateTime;
        File segno = new File(dest, ".versione");
        if (segno.isFile() && firma.equals(leggi(segno))) return dest;
        cancella(dest);
        copiaAsset(c, "webapp", dest);
        scriviPrivato(segno, firma);
        return dest;
    }

    private static void copiaAsset(Context c, String da, File a) throws Exception {
        String[] voci = c.getAssets().list(da);
        if (voci == null || voci.length == 0) {          // un file
            a.getParentFile().mkdirs();
            try (InputStream in = c.getAssets().open(da); OutputStream out = new FileOutputStream(a)) {
                byte[] buf = new byte[65536];
                int n;
                while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
            }
            return;
        }
        a.mkdirs();
        for (String v : voci) copiaAsset(c, da + "/" + v, new File(a, v));
    }

    private static void cancella(File f) {
        File[] figli = f.listFiles();
        if (figli != null) for (File x : figli) cancella(x);
        f.delete();
    }

    private static String leggi(File f) {
        try (InputStream in = new java.io.FileInputStream(f)) {
            byte[] b = new byte[(int) Math.min(f.length(), 4096)];
            int n = in.read(b);
            return new String(b, 0, Math.max(0, n), "UTF-8").trim();
        } catch (Exception e) {
            return "";
        }
    }

    /** Un file leggibile e scrivibile solo da questa app (600). */
    private static void scriviPrivato(File f, String testo) throws Exception {
        try (OutputStream o = new FileOutputStream(f)) {
            o.write(testo.getBytes("UTF-8"));
        }
        f.setReadable(false, false);
        f.setWritable(false, false);
        f.setReadable(true, true);
        f.setWritable(true, true);
    }
}
