package com.pairspan.app;
import java.lang.reflect.*;
import java.net.Socket;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
public class TunnelCheck {
 static Field field(String name)throws Exception {Field f=AdbTunnel.class.getDeclaredField(name); f.setAccessible(true); return f;}
 static void check(boolean value) {if(!value)throw new AssertionError();}
 public static void main(String[] args)throws Exception {
  var events=new java.util.ArrayList<org.json.JSONObject>();
  var tunnel=new AdbTunnel((event,payload)->events.add(payload));
  var socket=new Socket();
  field("socket").set(tunnel,socket);
  field("tunnelId").set(tunnel,"replacement");
  ((AtomicBoolean)field("open").get(tunnel)).set(true);
  Method close=AdbTunnel.class.getDeclaredMethod("sendClose",String.class,String.class); close.setAccessible(true);
  close.invoke(tunnel,"retired","old reader failed");
  tunnel.closeFromRemote("retired");
  check(!socket.isClosed()); check(events.isEmpty());
  var writer=(ExecutorService)field("writeWorker").get(tunnel);
  var gate=new CountDownLatch(1);
  writer.submit(()->{try {gate.await();}catch(InterruptedException e){Thread.currentThread().interrupt();}});
  tunnel.write("replacement","aGVsbG8=");
  field("tunnelId").set(tunnel,"newest");
  gate.countDown(); writer.submit(()->{}).get(2,TimeUnit.SECONDS);
  check(!socket.isClosed()); check(events.isEmpty());
  close.invoke(tunnel,"newest","EOF");
  check(socket.isClosed()); check(events.size()==1);
  check(events.get(0).values.get("id").equals("newest"));
  close.invoke(tunnel,"newest","duplicate"); check(events.size()==1);
  writer.shutdownNow(); ((ExecutorService)field("openWorker").get(tunnel)).shutdownNow();
  System.out.println("PASS: stale reader, remote close, queued write, and current EOF cleanup");
 }
}
