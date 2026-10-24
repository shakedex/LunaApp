// H.264 / HEVC and ProRes RAW frames through AVFoundation, for luna-decode's
// macOS backend (src/platform_macos.rs). One AVAssetReader per frame asked for,
// limited to a time range that starts inside the frame. Video comes back as
// the decoder's own bi-planar Y'CbCr, untouched (no colour conversion), as
// Media Foundation's does on Windows; ProRes RAW as the sensor mosaic ('bp16')
// with Apple's raw-conversion metadata.

#import <AVFoundation/AVFoundation.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <VideoToolbox/VideoToolbox.h>
#include <stdio.h>
#include <string.h>

enum { LUNA_AVF_OK = 0, LUNA_AVF_FAILED = 1, LUNA_AVF_UNSUPPORTED = 2, LUNA_AVF_NOT_FOUND = 3 };

typedef struct {
    double fps;      // nominal, 0 when unknown
    double start;    // seconds, presentation time of the first frame
    double duration; // seconds
    uint8_t transfer;   // ITU-T H.273 codes, 2 (unspecified) when unknown
    uint8_t primaries;
    int8_t full_range;  // 1 full, 0 video range
    const uint8_t *config; // avcC / hvcC record body, valid until luna_avf_close
    size_t config_len;
} luna_avf_info;

typedef struct {
    uint32_t pixel_format;
    uint32_t width, height; // luma plane
    uint32_t crop_x, crop_y, crop_w, crop_h;
    const uint8_t *planes[2];
    size_t bytes_per_row[2];
    size_t plane_height[2];
    uint8_t matrix; // ITU-T H.273, 2 when unknown
    void *handle;   // the locked CVPixelBuffer
} luna_avf_image;

typedef struct {
    uint32_t width, height;
    const uint16_t *data; // 16-bit sensels, one plane
    size_t bytes_per_row;
    int32_t bayer; // ProRes RAW bayer_pattern: 0 RGGB, 1 GRBG, 2 GBRG, 3 BGGR
    int32_t black, white;
    float wb_red, wb_blue; // multipliers for red- and blue-filtered sensels
    float camera_to_xyz[9]; // row-major, camera native RGB -> XYZ (D65)
    float gain;             // overall gain for raw conversion
    void *handle;           // the locked CVPixelBuffer
} luna_avf_raw;

@interface LunaAvfReader : NSObject
@property(nonatomic, strong) AVURLAsset *asset;
@property(nonatomic, strong) AVAssetTrack *track;
@property(nonatomic, strong) NSData *config;
@end
@implementation LunaAvfReader
@end

static void set_err(char *err, size_t len, NSString *what, NSError *e) {
    if (!err || !len) return;
    NSString *m = e ? [NSString stringWithFormat:@"%@: %@", what, e.localizedDescription] : what;
    snprintf(err, len, "%s", m.UTF8String ?: "");
}

static uint8_t code(int value) { return value > 0 && value < 256 ? (uint8_t)value : 2; }

void *luna_avf_open(const char *path, luna_avf_info *info, char *err, size_t err_len) {
    // ProRes RAW's decoder is one of the professional video workflow decoders,
    // which a process has to opt into.
    static dispatch_once_t pro;
    dispatch_once(&pro, ^{ VTRegisterProfessionalVideoWorkflowVideoDecoders(); });
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:url options:@{AVURLAssetPreferPreciseDurationAndTimingKey: @YES}];
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        __block NSArray<AVAssetTrack *> *tracks = nil;
        __block NSError *error = nil;
        [asset loadTracksWithMediaType:AVMediaTypeVideo
                     completionHandler:^(NSArray<AVAssetTrack *> *t, NSError *e) {
                       tracks = t;
                       error = e;
                       dispatch_semaphore_signal(done);
                     }];
        dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
        if (tracks.count == 0) {
            set_err(err, err_len, error ? @"AVFoundation could not open the file" : @"The file has no video track", error);
            return NULL;
        }
        AVAssetTrack *track = tracks.firstObject;
        NSArray *keys = @[@"formatDescriptions", @"timeRange", @"nominalFrameRate"];
        [track loadValuesAsynchronouslyForKeys:keys completionHandler:^{ dispatch_semaphore_signal(done); }];
        dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
        for (NSString *key in keys) {
            if ([track statusOfValueForKey:key error:&error] != AVKeyValueStatusLoaded) {
                set_err(err, err_len, @"AVFoundation could not read the video track", error);
                return NULL;
            }
        }
        LunaAvfReader *r = [LunaAvfReader new];
        r.asset = asset;
        r.track = track;
        memset(info, 0, sizeof *info);
        info->fps = track.nominalFrameRate;
        info->start = CMTimeGetSeconds(track.timeRange.start);
        info->duration = CMTimeGetSeconds(track.timeRange.duration);
        info->transfer = info->primaries = 2;
        CMFormatDescriptionRef fd = (__bridge CMFormatDescriptionRef)track.formatDescriptions.firstObject;
        if (fd) {
            CFTypeRef v = CMFormatDescriptionGetExtension(fd, kCMFormatDescriptionExtension_TransferFunction);
            if (v && CFGetTypeID(v) == CFStringGetTypeID())
                info->transfer = code(CVTransferFunctionGetIntegerCodePointForString((CFStringRef)v));
            v = CMFormatDescriptionGetExtension(fd, kCMFormatDescriptionExtension_ColorPrimaries);
            if (v && CFGetTypeID(v) == CFStringGetTypeID())
                info->primaries = code(CVColorPrimariesGetIntegerCodePointForString((CFStringRef)v));
            v = CMFormatDescriptionGetExtension(fd, kCMFormatDescriptionExtension_FullRangeVideo);
            info->full_range = v && CFGetTypeID(v) == CFBooleanGetTypeID() && CFBooleanGetValue((CFBooleanRef)v);
            CFTypeRef atoms = CMFormatDescriptionGetExtension(fd, kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms);
            if (atoms && CFGetTypeID(atoms) == CFDictionaryGetTypeID()) {
                NSDictionary *d = (__bridge NSDictionary *)atoms;
                id c = d[@"avcC"] ?: d[@"hvcC"];
                if ([c isKindOfClass:[NSArray class]]) c = [c firstObject];
                if ([c isKindOfClass:[NSData class]]) r.config = c;
            }
        }
        info->config = r.config.bytes;
        info->config_len = r.config.length;
        return (__bridge_retained void *)r;
    }
}

// The frame showing at `seconds`, decoded to `pixel_format`, retained and
// unlocked in `*out`. The reader's first sample is the one that overlaps the
// start of its time range (stamped with the range start, not its own time), so
// callers pass a time inside the frame rather than its exact start.
// LUNA_AVF_UNSUPPORTED: the decoder refused the stream before producing anything.
static int read_frame(LunaAvfReader *r, double seconds, uint32_t pixel_format, CVPixelBufferRef *out,
                      char *err, size_t err_len) {
    @autoreleasepool {
        NSError *error = nil;
        AVAssetReader *ar = [AVAssetReader assetReaderWithAsset:r.asset error:&error];
        if (!ar) {
            set_err(err, err_len, @"AVFoundation could not read the file", error);
            return LUNA_AVF_FAILED;
        }
        CMTimeRange whole = r.track.timeRange;
        CMTime from = CMTimeMakeWithSeconds(seconds, whole.duration.timescale ?: 600);
        if (CMTimeCompare(from, whole.start) < 0) from = whole.start;
        ar.timeRange = CMTimeRangeFromTimeToTime(from, CMTimeRangeGetEnd(whole));
        NSDictionary *settings = @{(id)kCVPixelBufferPixelFormatTypeKey: @(pixel_format)};
        AVAssetReaderTrackOutput *output = [AVAssetReaderTrackOutput assetReaderTrackOutputWithTrack:r.track
                                                                                      outputSettings:settings];
        output.alwaysCopiesSampleData = NO;
        if (![ar canAddOutput:output]) {
            set_err(err, err_len, @"AVFoundation can't decode this video", nil);
            return LUNA_AVF_UNSUPPORTED;
        }
        [ar addOutput:output];
        if (![ar startReading]) {
            set_err(err, err_len, @"AVFoundation could not start decoding", ar.error);
            return LUNA_AVF_UNSUPPORTED;
        }
        for (;;) {
            CMSampleBufferRef sample = [output copyNextSampleBuffer];
            if (!sample) break;
            CVImageBufferRef image = CMSampleBufferGetImageBuffer(sample);
            if (!image) {
                CFRelease(sample);
                continue;
            }
            *out = (CVPixelBufferRef)CFRetain(image);
            CFRelease(sample);
            [ar cancelReading];
            return LUNA_AVF_OK;
        }
        if (ar.status == AVAssetReaderStatusFailed) {
            set_err(err, err_len, @"AVFoundation could not decode the video", ar.error);
            return LUNA_AVF_UNSUPPORTED;
        }
        set_err(err, err_len, @"No frame at that time", nil);
        return LUNA_AVF_NOT_FOUND;
    }
}

int luna_avf_frame(void *reader, double seconds, uint32_t pixel_format, luna_avf_image *out,
                   char *err, size_t err_len) {
    @autoreleasepool {
        CVPixelBufferRef pb = NULL;
        int rc = read_frame((__bridge LunaAvfReader *)reader, seconds, pixel_format, &pb, err, err_len);
        if (rc != LUNA_AVF_OK) return rc;
        if (!CVPixelBufferIsPlanar(pb) || CVPixelBufferGetPlaneCount(pb) != 2 ||
            CVPixelBufferLockBaseAddress(pb, kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess) {
            CFRelease(pb);
            set_err(err, err_len, @"AVFoundation returned an unexpected frame layout", nil);
            return LUNA_AVF_FAILED;
        }
        memset(out, 0, sizeof *out);
        out->pixel_format = CVPixelBufferGetPixelFormatType(pb);
        out->width = (uint32_t)CVPixelBufferGetWidth(pb);
        out->height = (uint32_t)CVPixelBufferGetHeight(pb);
        CGRect clean = CVImageBufferGetCleanRect(pb);
        out->crop_x = (uint32_t)clean.origin.x;
        out->crop_y = (uint32_t)clean.origin.y;
        out->crop_w = (uint32_t)clean.size.width;
        out->crop_h = (uint32_t)clean.size.height;
        for (size_t p = 0; p < 2; p++) {
            out->planes[p] = CVPixelBufferGetBaseAddressOfPlane(pb, p);
            out->bytes_per_row[p] = CVPixelBufferGetBytesPerRowOfPlane(pb, p);
            out->plane_height[p] = CVPixelBufferGetHeightOfPlane(pb, p);
        }
        out->matrix = 2;
        CFTypeRef m = CVBufferCopyAttachment(pb, kCVImageBufferYCbCrMatrixKey, NULL);
        if (m) {
            if (CFGetTypeID(m) == CFStringGetTypeID())
                out->matrix = code(CVYCbCrMatrixGetIntegerCodePointForString((CFStringRef)m));
            CFRelease(m);
        }
        out->handle = pb;
        return LUNA_AVF_OK;
    }
}

static int number(CVPixelBufferRef pb, CFStringRef key, CFNumberType type, void *value) {
    CFTypeRef v = CVBufferCopyAttachment(pb, key, NULL);
    int ok = v && CFGetTypeID(v) == CFNumberGetTypeID() && CFNumberGetValue((CFNumberRef)v, type, value);
    if (v) CFRelease(v);
    return ok;
}

// A ProRes RAW frame as its 16-bit sensor mosaic, with the metadata Apple
// requires on 'bp16' buffers (CVPixelBuffer.h): levels, white balance, the
// camera -> XYZ matrix and the gain.
int luna_avf_raw_frame(void *reader, double seconds, luna_avf_raw *out, char *err, size_t err_len) {
    @autoreleasepool {
        CVPixelBufferRef pb = NULL;
        int rc = read_frame((__bridge LunaAvfReader *)reader, seconds, kCVPixelFormatType_16VersatileBayer, &pb,
                            err, err_len);
        if (rc != LUNA_AVF_OK) return rc;
        memset(out, 0, sizeof *out);
        int ok = CVPixelBufferGetPixelFormatType(pb) == kCVPixelFormatType_16VersatileBayer &&
                 !CVPixelBufferIsPlanar(pb) &&
                 number(pb, kCVPixelBufferVersatileBayerKey_BayerPattern, kCFNumberSInt32Type, &out->bayer) &&
                 number(pb, kCVPixelBufferProResRAWKey_BlackLevel, kCFNumberSInt32Type, &out->black) &&
                 number(pb, kCVPixelBufferProResRAWKey_WhiteLevel, kCFNumberSInt32Type, &out->white) &&
                 number(pb, kCVPixelBufferProResRAWKey_WhiteBalanceRedFactor, kCFNumberFloat32Type, &out->wb_red) &&
                 number(pb, kCVPixelBufferProResRAWKey_WhiteBalanceBlueFactor, kCFNumberFloat32Type, &out->wb_blue) &&
                 number(pb, kCVPixelBufferProResRAWKey_GainFactor, kCFNumberFloat32Type, &out->gain);
        CFTypeRef m = ok ? CVBufferCopyAttachment(pb, kCVPixelBufferProResRAWKey_ColorMatrix, NULL) : NULL;
        ok = m && CFGetTypeID(m) == CFDataGetTypeID() && CFDataGetLength((CFDataRef)m) == sizeof out->camera_to_xyz;
        if (ok) memcpy(out->camera_to_xyz, CFDataGetBytePtr((CFDataRef)m), sizeof out->camera_to_xyz);
        if (m) CFRelease(m);
        if (!ok || CVPixelBufferLockBaseAddress(pb, kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess) {
            CFRelease(pb);
            set_err(err, err_len, @"AVFoundation returned a ProRes RAW frame without its mosaic or metadata", nil);
            return LUNA_AVF_FAILED;
        }
        out->width = (uint32_t)CVPixelBufferGetWidth(pb);
        out->height = (uint32_t)CVPixelBufferGetHeight(pb);
        out->data = CVPixelBufferGetBaseAddress(pb);
        out->bytes_per_row = CVPixelBufferGetBytesPerRow(pb);
        out->handle = pb;
        return LUNA_AVF_OK;
    }
}

void luna_avf_raw_release(luna_avf_raw *raw) {
    if (!raw || !raw->handle) return;
    CVPixelBufferRef pb = raw->handle;
    CVPixelBufferUnlockBaseAddress(pb, kCVPixelBufferLock_ReadOnly);
    CFRelease(pb);
    raw->handle = NULL;
}

void luna_avf_image_release(luna_avf_image *image) {
    if (!image || !image->handle) return;
    CVPixelBufferRef pb = image->handle;
    CVPixelBufferUnlockBaseAddress(pb, kCVPixelBufferLock_ReadOnly);
    CFRelease(pb);
    image->handle = NULL;
}

void luna_avf_close(void *reader) {
    if (reader) CFRelease(reader);
}
