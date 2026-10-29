// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
package org.nyctalus.mobile;

import android.content.Context;

import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.net.SocketTimeoutException;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;

/**
 * Passa a Termux il pacchetto di Nyctalus che l'app porta con sé (assets).
 *
 * Termux non può leggere i file interni dell'app e non vogliamo chiedere il permesso
 * dell'archivio: per qualche minuto l'app serve il pacchetto su 127.0.0.1, a una porta
 * libera e a un indirizzo casuale (128 bit), e Termux lo scarica con curl. Il comando
 * di installazione controlla anche l'impronta SHA-256, quindi un altro programma non
 * potrebbe sostituire il pacchetto. Serve solo quel file, poi si spegne da solo.
 */
class ServitorePacchetto extends Thread {

    static final String NOME = "nyctalus-termux.tar.gz";
    private static final long DURATA_MS = 15 * 60 * 1000;

    private final Context ctx;
    private final ServerSocket server;
    final String percorso, impronta;
    final int porta;

    ServitorePacchetto(Context ctx) throws Exception {
        this.ctx = ctx.getApplicationContext();
        byte[] b = new byte[16];
        new SecureRandom().nextBytes(b);
        percorso = esadecimale(b);
        MessageDigest md = MessageDigest.getInstance("SHA-256");
        try (InputStream in = ctx.getAssets().open(NOME)) {
            byte[] buf = new byte[65536];
            int n;
            while ((n = in.read(buf)) > 0) md.update(buf, 0, n);
        }
        impronta = esadecimale(md.digest());
        server = new ServerSocket(0, 4, InetAddress.getByName("127.0.0.1"));
        server.setSoTimeout(2000);
        porta = server.getLocalPort();
        setDaemon(true);
    }

    /** True se l'app contiene il pacchetto (le compilazioni con Gradle potrebbero non averlo). */
    static boolean presente(Context ctx) {
        try (InputStream in = ctx.getAssets().open(NOME)) {
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    String indirizzo() {
        return "http://127.0.0.1:" + porta + "/" + percorso;
    }

    @Override
    public void run() {
        long fine = System.currentTimeMillis() + DURATA_MS;
        try {
            while (System.currentTimeMillis() < fine) {
                try (Socket s = server.accept()) {
                    s.setSoTimeout(5000);
                    servi(s);
                } catch (SocketTimeoutException attesa) {
                    // nessuno: si ricontrolla la scadenza
                } catch (Exception ignorata) { }
            }
        } finally {
            try { server.close(); } catch (Exception ignorata) { }
        }
    }

    private void servi(Socket s) throws Exception {
        InputStream in = s.getInputStream();
        StringBuilder riga = new StringBuilder();
        int c;
        while ((c = in.read()) != -1 && c != '\n' && riga.length() < 512) riga.append((char) c);
        OutputStream out = s.getOutputStream();
        if (!riga.toString().startsWith("GET /" + percorso + " ")) {
            out.write("HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\n\r\n".getBytes(StandardCharsets.US_ASCII));
            return;
        }
        out.write(("HTTP/1.0 200 OK\r\nContent-Type: application/gzip\r\nConnection: close\r\n\r\n")
                .getBytes(StandardCharsets.US_ASCII));
        try (InputStream a = ctx.getAssets().open(NOME)) {
            byte[] buf = new byte[65536];
            int n;
            while ((n = a.read(buf)) > 0) out.write(buf, 0, n);
        }
        out.flush();
    }

    private static String esadecimale(byte[] b) {
        StringBuilder sb = new StringBuilder();
        for (byte x : b) sb.append(String.format("%02x", x));
        return sb.toString();
    }
}
