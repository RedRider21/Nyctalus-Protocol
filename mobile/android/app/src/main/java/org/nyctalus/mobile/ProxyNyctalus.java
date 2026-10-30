// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
package org.nyctalus.mobile;

import android.os.Build;
import android.webkit.WebView;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.util.Collection;
import java.util.concurrent.Executor;

/**
 * Collega le WebView del browser alla rete Nyctalus, senza librerie esterne.
 *
 * <p>La WebView di Android non ha un'impostazione pubblica del proxy nel framework: ce
 * l'ha la WebView di Chrome stessa, attraverso le «boundary interface» che la libreria
 * androidx.webkit usa per {@code ProxyController} e {@code addDocumentStartJavaScript}.
 * Qui si fanno le stesse chiamate direttamente, via reflection sul classloader della
 * WebView: è la stessa strada ufficiale, senza portarsi dietro la libreria.
 *
 * <p>Tutto quello che qui non riesce lo dice ({@code false} o eccezione): chi chiama deve
 * NON navigare in modalità Nyctalus se il proxy non è attivo e verificato.
 */
final class ProxyNyctalus {

    private static final String B = "org.chromium.support_lib_boundary.";

    private ProxyNyctalus() { }

    /** Le funzioni che dichiariamo di saper usare (come fa androidx.webkit): la WebView
     *  abilita solo quelle che sanno usare entrambe le parti. */
    private static final String[] NOSTRE = {"PROXY_OVERRIDE", "PROXY_OVERRIDE_REVERSE_BYPASS", "DOCUMENT_START_SCRIPT"};

    private static InvocationHandler fabbrica;
    /** Perché l'ultima operazione non è riuscita (per il messaggio all'utente). */
    static String ultimoErrore = "";

    /** La fabbrica della WebView di Chrome (InvocationHandler), oppure eccezione. */
    private static synchronized InvocationHandler fabbrica() throws Exception {
        if (fabbrica != null) return fabbrica;
        if (Build.VERSION.SDK_INT < 28) throw new IllegalStateException("serve Android 9 o successivo");
        ClassLoader cl = WebView.getWebViewClassLoader();
        Class<?> glue = Class.forName("org.chromium.support_lib_glue.SupportLibReflectionUtil", false, cl);
        // La forma di createWebViewProviderFactory cambia fra le versioni della WebView:
        // con un argomento (la «libreria», un oggetto che risponde a getSupportedFeatures,
        // come fa androidx) o senza. Invece di indovinare, si guarda quale c'è.
        InvocationHandler libreria = (proxy, m, argomenti) ->
                "getSupportedFeatures".equals(m.getName()) ? NOSTRE.clone() : null;
        StringBuilder trovati = new StringBuilder();
        for (Method m : glue.getDeclaredMethods()) {
            trovati.append(m.getName()).append(java.util.Arrays.toString(m.getParameterTypes())).append(' ');
            if (!"createWebViewProviderFactory".equals(m.getName())) continue;
            m.setAccessible(true);
            Class<?>[] tipi = m.getParameterTypes();
            Object f = null;
            if (tipi.length == 0) f = m.invoke(null);
            else if (tipi.length == 1 && tipi[0].isAssignableFrom(InvocationHandler.class)) f = m.invoke(null, libreria);
            if (f instanceof InvocationHandler) { fabbrica = (InvocationHandler) f; break; }
        }
        if (fabbrica == null) throw new NoSuchMethodException("createWebViewProviderFactory non trovato; metodi: " + trovati);
        return fabbrica;
    }

    private static Method metodo(String interfaccia, String nome, Class<?>... tipi) throws Exception {
        return Class.forName(B + interfaccia, false, WebView.getWebViewClassLoader()).getDeclaredMethod(nome, tipi);
    }

    private static Object chiama(InvocationHandler h, Method m, Object... argomenti) throws Throwable {
        return h.invoke(null, m, argomenti.length == 0 ? null : argomenti);
    }

    /** Le funzioni che questa WebView offre (es. PROXY_OVERRIDE, DOCUMENT_START_SCRIPT). */
    static boolean supporta(String funzione) {
        try {
            Object f = chiama(fabbrica(), metodo("WebViewProviderFactoryBoundaryInterface", "getSupportedFeatures"));
            if (f instanceof String[]) {
                for (String s : (String[]) f) if (funzione.equals(s)) return true;
            } else if (f instanceof Collection) {
                return ((Collection<?>) f).contains(funzione);
            }
            ultimoErrore = "la WebView non offre " + funzione;
        } catch (Throwable e) {
            Throwable c = e.getCause() != null ? e.getCause() : e;
            ultimoErrore = c.getClass().getSimpleName() + ": " + c.getMessage();
        }
        return false;
    }

    /**
     * Tutto il traffico delle WebView di QUESTO processo passa dal SOCKS indicato, anche
     * quello verso 127.0.0.1 (regola «&lt;-loopback&gt;»: i siti non raggiungono i servizi
     * del telefono). {@code fatto} gira quando il proxy è in vigore.
     */
    static void imposta(String host, int porta, Runnable fatto, Executor esecutore) throws Throwable {
        InvocationHandler pc = (InvocationHandler) chiama(fabbrica(),
                metodo("WebViewProviderFactoryBoundaryInterface", "getProxyController"));
        String[][] regole = {{"*", "socks5://" + host + ":" + porta}};
        String[] eccezioni = {"<-loopback>"};
        try {
            chiama(pc, metodo("ProxyControllerBoundaryInterface", "setProxyOverride",
                    String[][].class, String[].class, Runnable.class, Executor.class, boolean.class),
                    regole, eccezioni, fatto, esecutore, false);
        } catch (NoSuchMethodException vecchia) {
            chiama(pc, metodo("ProxyControllerBoundaryInterface", "setProxyOverride",
                    String[][].class, String[].class, Runnable.class, Executor.class),
                    regole, eccezioni, fatto, esecutore);
        }
    }

    /** Toglie il proxy: le WebView di questo processo tornano a collegarsi direttamente. */
    static void togli(Runnable fatto, Executor esecutore) throws Throwable {
        InvocationHandler pc = (InvocationHandler) chiama(fabbrica(),
                metodo("WebViewProviderFactoryBoundaryInterface", "getProxyController"));
        chiama(pc, metodo("ProxyControllerBoundaryInterface", "clearProxyOverride", Runnable.class, Executor.class),
                fatto, esecutore);
    }

    /**
     * Uno script che gira in ogni pagina e in ogni riquadro PRIMA degli script del sito
     * (come androidx {@code addDocumentStartJavaScript}). True se è stato installato.
     */
    static boolean scriptAllAvvio(WebView web, String script) {
        try {
            InvocationHandler provider = (InvocationHandler) chiama(fabbrica(),
                    metodo("WebViewProviderFactoryBoundaryInterface", "createWebView", WebView.class), web);
            chiama(provider, metodo("WebViewProviderBoundaryInterface", "addDocumentStartJavaScript",
                    String.class, String[].class), script, new String[]{"*"});
            return true;
        } catch (Throwable e) {
            return false;
        }
    }
}
