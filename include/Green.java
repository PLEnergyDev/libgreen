public class Green {
    static {
        System.loadLibrary("green");
    }

    public native long measureStart(String events);
    public native void measureStop(long handle);
}

