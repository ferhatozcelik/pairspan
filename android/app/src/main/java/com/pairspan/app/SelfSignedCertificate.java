package com.pairspan.app;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.Signature;
import java.security.cert.Certificate;
import java.security.cert.CertificateFactory;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.Locale;
import java.util.TimeZone;

/** Builds a self-signed X.509 v3 certificate (SHA256withRSA) with a hand-written DER encoder. */
final class SelfSignedCertificate {
    private static final byte[] SHA256_RSA_OID = {0x2A, (byte) 0x86, 0x48, (byte) 0x86, (byte) 0xF7, 0x0D, 0x01, 0x01, 0x0B};
    private static final byte[] COMMON_NAME_OID = {0x55, 0x04, 0x03};

    private SelfSignedCertificate() { }

    static Certificate create(KeyPair pair, String commonName, Date start, Date end, BigInteger serial) throws Exception {
        byte[] algorithm = der(0x30, der(0x06, SHA256_RSA_OID), new byte[] {0x05, 0x00});
        byte[] name = der(0x30, der(0x31, der(0x30, der(0x06, COMMON_NAME_OID),
                der(0x0C, commonName.getBytes(StandardCharsets.UTF_8)))));
        byte[] tbs = der(0x30,
                der(0xA0, der(0x02, new byte[] {0x02})),
                der(0x02, serial.toByteArray()),
                algorithm,
                name,
                der(0x30, utcTime(start), utcTime(end)),
                name,
                pair.getPublic().getEncoded());
        Signature signature = Signature.getInstance("SHA256withRSA");
        signature.initSign(pair.getPrivate());
        signature.update(tbs);
        byte[] value = signature.sign();
        byte[] bitString = new byte[value.length + 1];
        System.arraycopy(value, 0, bitString, 1, value.length);
        byte[] certificate = der(0x30, tbs, algorithm, der(0x03, bitString));
        return CertificateFactory.getInstance("X.509").generateCertificate(new ByteArrayInputStream(certificate));
    }

    private static byte[] utcTime(Date date) {
        SimpleDateFormat format = new SimpleDateFormat("yyMMddHHmmss'Z'", Locale.US);
        format.setTimeZone(TimeZone.getTimeZone("UTC"));
        return der(0x17, format.format(date).getBytes(StandardCharsets.US_ASCII));
    }

    private static byte[] der(int tag, byte[]... parts) {
        ByteArrayOutputStream body = new ByteArrayOutputStream();
        for (byte[] part : parts) body.write(part, 0, part.length);
        byte[] content = body.toByteArray();
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(tag);
        if (content.length < 0x80) out.write(content.length);
        else if (content.length < 0x100) { out.write(0x81); out.write(content.length); }
        else { out.write(0x82); out.write(content.length >> 8); out.write(content.length & 0xFF); }
        out.write(content, 0, content.length);
        return out.toByteArray();
    }
}
