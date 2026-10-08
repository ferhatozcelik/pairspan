package com.pairspan.app;

import android.content.Context;
import android.os.Build;
import java.io.*;
import java.math.BigInteger;
import java.security.*;
import java.security.cert.Certificate;
import java.security.cert.CertificateFactory;
import java.security.spec.PKCS8EncodedKeySpec;
import java.util.Date;
import java.util.Random;
import com.pairspan.app.adb.AbsAdbConnectionManager;

/** The ADB identity belongs to Pairspan and survives app restarts. */
final class LocalAdbManager extends AbsAdbConnectionManager {
    private final PrivateKey privateKey;
    private final Certificate certificate;

    LocalAdbManager(Context context) throws Exception {
        setApi(Build.VERSION.SDK_INT);
        setHostAddress("127.0.0.1");
        File keyFile = new File(context.getFilesDir(), "adb-private.key");
        File certFile = new File(context.getFilesDir(), "adb-cert.der");
        if (keyFile.exists() && certFile.exists()) {
            privateKey = KeyFactory.getInstance("RSA").generatePrivate(new PKCS8EncodedKeySpec(read(keyFile)));
            certificate = CertificateFactory.getInstance("X.509").generateCertificate(new ByteArrayInputStream(read(certFile)));
        } else {
            KeyPairGenerator generator = KeyPairGenerator.getInstance("RSA");
            generator.initialize(2048, new SecureRandom());
            KeyPair pair = generator.generateKeyPair();
            privateKey = pair.getPrivate();
            Date start = new Date(System.currentTimeMillis() - 86400000L);
            Date end = new Date(System.currentTimeMillis() + 20L * 365 * 86400000L);
            certificate = SelfSignedCertificate.create(pair, "Pairspan", start, end,
                    BigInteger.valueOf(new Random().nextInt() & Integer.MAX_VALUE));
            write(keyFile, privateKey.getEncoded());
            write(certFile, certificate.getEncoded());
        }
    }

    private static byte[] read(File file) throws IOException {
        try (FileInputStream in = new FileInputStream(file)) { return in.readAllBytes(); }
    }
    private static void write(File file, byte[] bytes) throws IOException {
        try (FileOutputStream out = new FileOutputStream(file)) { out.write(bytes); }
    }
    @Override protected PrivateKey getPrivateKey() { return privateKey; }
    @Override protected Certificate getCertificate() { return certificate; }
    @Override protected String getDeviceName() { return "Pairspan"; }
}
