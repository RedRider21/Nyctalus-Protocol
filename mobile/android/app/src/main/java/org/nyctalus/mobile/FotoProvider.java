// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
package org.nyctalus.mobile;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;

import java.io.File;
import java.io.FileNotFoundException;

/**
 * Fornitore minimo per le foto e i video scattati dalla chat, e per i file dell'archivio di
 * Nyctalus da APRIRE o CONDIVIDERE con un'altra app (cache/aperti: copia temporanea fatta
 * dall'app, che è in primo piano — Termux in sottofondo non può aprire schermate).
 *
 * La fotocamera del telefono scrive nel file che le passiamo (EXTRA_OUTPUT) e il
 * file resta nella cartella PRIVATA dell'app (cache/scatti): non finisce nella
 * galleria e nessun'altra app lo vede. Il permesso di scrittura è temporaneo e
 * vale solo per quell'unico indirizzo. Espone solo nomi semplici dentro
 * cache/scatti (niente percorsi): "../" e simili vengono rifiutati.
 */
public class FotoProvider extends ContentProvider {

    static final String AUTORITA = "org.nyctalus.mobile.foto";

    static File cartella(android.content.Context c) {
        File d = new File(c.getCacheDir(), "scatti");
        d.mkdirs();
        return d;
    }

    static Uri indirizzo(String nome) {
        return Uri.parse("content://" + AUTORITA + "/" + nome);
    }

    /** Copie temporanee dei file da aprire o condividere (si buttano dopo un giorno). */
    static File cartellaAperti(android.content.Context c) {
        File d = new File(c.getCacheDir(), "aperti");
        d.mkdirs();
        return d;
    }

    static Uri indirizzoAperto(String nome) {
        return Uri.parse("content://" + AUTORITA + "/aperti/" + nome);
    }

    private File file(Uri uri) throws FileNotFoundException {
        java.util.List<String> parti = uri.getPathSegments();
        String nome = uri.getLastPathSegment();
        if (nome == null || !nome.matches("[A-Za-z0-9_.-]{1,120}") || nome.startsWith(".")) {
            throw new FileNotFoundException("nome non valido");
        }
        if (parti.size() == 2 && "aperti".equals(parti.get(0))) return new File(cartellaAperti(getContext()), nome);
        if (parti.size() != 1) throw new FileNotFoundException("percorso non valido");
        return new File(cartella(getContext()), nome);
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        return ParcelFileDescriptor.open(file(uri), ParcelFileDescriptor.parseMode(mode));
    }

    @Override
    public String getType(Uri uri) {
        String n = uri.getLastPathSegment();
        if (n == null) return null;
        int punto = n.lastIndexOf('.');
        String t = punto < 0 ? null
                : android.webkit.MimeTypeMap.getSingleton().getMimeTypeFromExtension(n.substring(punto + 1).toLowerCase());
        return t != null ? t : "application/octet-stream";
    }

    @Override
    public Cursor query(Uri uri, String[] proiezione, String sel, String[] args, String ord) {
        File f;
        try { f = file(uri); } catch (FileNotFoundException e) { return null; }
        MatrixCursor c = new MatrixCursor(new String[]{OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE});
        c.addRow(new Object[]{f.getName(), f.length()});
        return c;
    }

    @Override
    public Uri insert(Uri uri, ContentValues v) { return null; }

    @Override
    public int delete(Uri uri, String sel, String[] args) { return 0; }

    @Override
    public int update(Uri uri, ContentValues v, String sel, String[] args) { return 0; }
}
