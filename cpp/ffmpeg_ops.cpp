#include "bimg.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

extern "C" {

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>
#include <libavutil/imgutils.h>
#include <libavutil/mem.h>
#include <libswscale/swscale.h>

}

struct MemCtx {
    const uint8_t* data;
    size_t size;
    size_t pos;
};

struct DecodedFrame {
    double pts;
    int width;
    int height;
    std::vector<uint8_t> rgb;
};

static int mem_read(void* opaque, uint8_t* buf, int buf_size) {
    MemCtx* m = static_cast<MemCtx*>(opaque);
    size_t remaining = (m->pos < m->size) ? (m->size - m->pos) : 0;
    int n = (remaining < static_cast<size_t>(buf_size)) ? static_cast<int>(remaining) : buf_size;
    if (n > 0) {
        std::memcpy(buf, m->data + m->pos, static_cast<size_t>(n));
        m->pos += static_cast<size_t>(n);
    }
    return n > 0 ? n : AVERROR_EOF;
}

static int64_t mem_seek(void* opaque, int64_t offset, int whence) {
    MemCtx* m = static_cast<MemCtx*>(opaque);
    if (whence == AVSEEK_SIZE) {
        return static_cast<int64_t>(m->size);
    }
    int64_t newpos = m->pos;
    if (whence == SEEK_SET) {
        newpos = offset;
    } else if (whence == SEEK_CUR) {
        newpos = static_cast<int64_t>(m->pos) + offset;
    } else if (whence == SEEK_END) {
        newpos = static_cast<int64_t>(m->size) + offset;
    }
    if (newpos < 0 || static_cast<size_t>(newpos) > m->size) {
        return -1;
    }
    m->pos = static_cast<size_t>(newpos);
    return m->pos;
}

static bool decode_all(const uint8_t* data, size_t len, size_t max_duration_ms, int target_w, std::vector<DecodedFrame>& decoded) {
    if (!data || !len) {
        return false;
    }

    MemCtx mem{data, len, 0};
    uint8_t* avio_buf = static_cast<uint8_t*>(av_malloc(4096));
    if (!avio_buf) {
        return false;
    }

    AVIOContext* io = avio_alloc_context(avio_buf, 4096, 0, &mem, mem_read, nullptr, mem_seek);
    if (!io) {
        av_freep(&avio_buf);
        return false;
    }

    AVFormatContext* fmt = avformat_alloc_context();
    if (!fmt) {
        if (io) {
            av_freep(&io->buffer);
            avio_context_free(&io);
        }
        return false;
    }
    fmt->flags |= AVFMT_FLAG_GENPTS;
    fmt->pb = io;
    fmt->io_open = nullptr;

    AVDictionary* opts = nullptr;
    if (avformat_open_input(&fmt, nullptr, nullptr, &opts) < 0 || !fmt) {
        if (io) {
            av_freep(&io->buffer);
            avio_context_free(&io);
        }
        return false;
    }

    if (avformat_find_stream_info(fmt, nullptr) < 0) {
        avformat_close_input(&fmt);
    if (io) {
        av_freep(&io->buffer);
        avio_context_free(&io);
    }
    return false;
    }

    int video_index = -1;
    for (unsigned i = 0; i < fmt->nb_streams; i++) {
        if (fmt->streams[i]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
            video_index = static_cast<int>(i);
            break;
        }
    }
    if (video_index < 0) {
        avformat_close_input(&fmt);
    if (io) {
        av_freep(&io->buffer);
        avio_context_free(&io);
    }
    return false;
    }

    AVStream* stream = fmt->streams[video_index];
    const AVCodec* dec = avcodec_find_decoder(stream->codecpar->codec_id);
    if (!dec) {
        avformat_close_input(&fmt);
    if (io) {
        av_freep(&io->buffer);
        avio_context_free(&io);
    }
    return false;
    }

    AVCodecContext* codec = avcodec_alloc_context3(dec);
    if (!codec || avcodec_parameters_to_context(codec, stream->codecpar) < 0 || avcodec_open2(codec, dec, nullptr) < 0) {
        if (codec) avcodec_free_context(&codec);
        avformat_close_input(&fmt);
    if (io) {
        av_freep(&io->buffer);
        avio_context_free(&io);
    }
    return false;
    }

    codec->thread_count = 1;

    bool sws_setup = false;
    SwsContext* sws = nullptr;
    int src_w = 0;
    int src_h = 0;
    enum AVPixelFormat src_pix_fmt = AV_PIX_FMT_NONE;

    AVPacket* pkt = av_packet_alloc();
    AVFrame* frame = av_frame_alloc();
    AVFrame* rgb = av_frame_alloc();

    enum class FrameStatus {
        Pushed,
        Skipped,
        Stop,
        Error,
    };

    auto process_decoded_frame = [&](AVFrame* frame) -> FrameStatus {
        double pts = 0.0;
        if (frame->best_effort_timestamp != AV_NOPTS_VALUE) {
            pts = av_q2d(stream->time_base) * static_cast<double>(frame->best_effort_timestamp);
        } else if (frame->pts != AV_NOPTS_VALUE) {
            pts = av_q2d(stream->time_base) * static_cast<double>(frame->pts);
        }
        if (pts < 0) {
            pts = static_cast<double>(decoded.size()) / 12.0;
        }

        if (pts > static_cast<double>(max_duration_ms) / 1000.0) {
            return FrameStatus::Stop;
        }

        if (frame->width <= 0 || frame->height <= 0 || frame->format < 0) {
            return FrameStatus::Skipped;
        }

        if (!sws_setup || frame->width != src_w || frame->height != src_h || frame->format != src_pix_fmt) {
            if (sws) {
                sws_freeContext(sws);
                sws = nullptr;
                sws_setup = false;
            }
            src_w = frame->width;
            src_h = frame->height;
            src_pix_fmt = static_cast<enum AVPixelFormat>(frame->format);
            int dst_h = (static_cast<long long>(src_h) * target_w) / src_w;
            if (dst_h < 1) {
                dst_h = 1;
            }
            sws = sws_getContext(
                src_w,
                src_h,
                src_pix_fmt,
                target_w,
                dst_h,
                AV_PIX_FMT_RGB8,
                SWS_BILINEAR,
                nullptr,
                nullptr,
                nullptr
            );
            if (!sws) {
                return FrameStatus::Error;
            }
            sws_setup = true;
        }

        if (rgb->data[0]) {
            av_frame_unref(rgb);
        }
        rgb->format = AV_PIX_FMT_RGB8;
        rgb->width = target_w;
        rgb->height = (static_cast<int>(static_cast<long long>(src_h) * target_w / src_w));
        if (rgb->height < 1) {
            rgb->height = 1;
        }
        if (av_frame_get_buffer(rgb, 32) < 0) {
            return FrameStatus::Skipped;
        }

        sws_scale(sws, frame->data, frame->linesize, 0, src_h, rgb->data, rgb->linesize);

        size_t rgb_bytes = static_cast<size_t>(rgb->width) * rgb->height * 1;
        DecodedFrame out;
        out.pts = pts;
        out.width = rgb->width;
        out.height = rgb->height;
        out.rgb.assign(rgb->data[0], rgb->data[0] + rgb_bytes);
        decoded.push_back(std::move(out));
        return FrameStatus::Pushed;
    };

    bool hard_error = false;
    bool finished = false;
    while (true) {
        bool next_frame = false;
        int ret = av_read_frame(fmt, pkt);
        if (ret < 0) {
            if (ret == AVERROR_EOF) {
                finished = true;
                if (avcodec_send_packet(codec, nullptr) >= 0) {
                    while (true) {
                        ret = avcodec_receive_frame(codec, frame);
                        if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
                            break;
                        }
                        if (ret < 0) {
                            hard_error = true;
                            break;
                        }
                        FrameStatus st = process_decoded_frame(frame);
                        av_frame_unref(frame);
                        if (st != FrameStatus::Pushed) {
                            if (st == FrameStatus::Error) {
                                hard_error = true;
                            }
                            next_frame = true;
                            break;
                        }
                    }
                }
            } else {
                hard_error = true;
            }
            break;
        }

        if (pkt->stream_index != video_index) {
            av_packet_unref(pkt);
            continue;
        }

        ret = avcodec_send_packet(codec, pkt);
        av_packet_unref(pkt);

        while (true) {
            ret = avcodec_receive_frame(codec, frame);
            if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
                break;
            }
            if (ret < 0) {
                hard_error = true;
                break;
            }

            FrameStatus st = process_decoded_frame(frame);
            av_frame_unref(frame);
            if (st == FrameStatus::Pushed) {
                continue;
            }
            if (st == FrameStatus::Error) {
                hard_error = true;
                break;
            }
            if (st == FrameStatus::Stop) {
                finished = true;
                next_frame = true;
                break;
            }
        }

        if (next_frame || finished || hard_error) {
            break;
        }
    }

    av_packet_free(&pkt);
    av_frame_free(&frame);
    av_frame_free(&rgb);
    if (sws) {
        sws_freeContext(sws);
    }
    avcodec_free_context(&codec);
    avformat_close_input(&fmt);
    if (io) {
        av_freep(&io->buffer);
        avio_context_free(&io);
    }

    if (hard_error && decoded.empty()) {
        return false;
    }
    return !decoded.empty();
}

imgbuf vid2gif(const uint8_t* data, size_t len) {
    imgbuf nil;
    nil.data = nullptr;
    nil.len = 0;

    std::vector<DecodedFrame> decoded;
    if (!decode_all(data, len, 30000, 480, decoded)) {
        return nil;
    }

    std::vector<DecodedFrame> selected;
    size_t cursor = 0;
    const size_t max_frames = 360;
    double start_pts = decoded.front().pts;
    double last_pts = decoded.back().pts;
    double frame_duration = decoded.size() > 1 ? (decoded[1].pts - decoded[0].pts) : (1.0 / 12.0);
    if (frame_duration <= 0.0) {
        frame_duration = 1.0 / 12.0;
    }
    double duration = (last_pts - start_pts) + frame_duration;
    if (duration < 0.05) {
        duration = static_cast<double>(decoded.size()) / 12.0;
    }
    duration = std::min(duration, 30.0);
    size_t frame_count = static_cast<size_t>(duration * 12.0 + 0.5);
    frame_count = std::max<size_t>(frame_count, 1);
    frame_count = std::min(frame_count, max_frames);
    selected.reserve(frame_count);
    for (size_t i = 0; i < frame_count; i++) {
        double target = static_cast<double>(i) / 12.0;
        while (cursor + 1 < decoded.size() && decoded[cursor + 1].pts <= target) {
            cursor++;
        }
        selected.push_back(decoded[cursor]);
    }
    if (selected.empty()) {
        return nil;
    }

    const AVCodec* enc = avcodec_find_encoder(AV_CODEC_ID_GIF);
    if (!enc) {
        return nil;
    }

    int w = selected.front().width;
    int h = selected.front().height;
    for (const auto& frame : selected) {
        if (frame.width != w || frame.height != h) {
            // normalize with rescale at decode time is assumed; keep first size
            selected = std::vector<DecodedFrame>();
            break;
        }
    }
    if (selected.empty()) {
        return nil;
    }

    AVDictionary* opts = nullptr;
    AVFormatContext* oc = nullptr;
    if (avformat_alloc_output_context2(&oc, nullptr, "gif", nullptr) < 0 || !oc) {
        return nil;
    }
    AVStream* stream = avformat_new_stream(oc, enc);
    if (!stream) {
        avformat_free_context(oc);
        return nil;
    }

    AVCodecContext* ctx = avcodec_alloc_context3(enc);
    if (!ctx) {
        avformat_free_context(oc);
        return nil;
    }
    ctx->width = w;
    ctx->height = h;
    ctx->time_base = (AVRational){1, 12};
    ctx->pix_fmt = AV_PIX_FMT_RGB8;
    ctx->thread_count = 1;
    if (oc->oformat && (oc->oformat->flags & AVFMT_GLOBALHEADER)) {
        ctx->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
    }
    if (avcodec_open2(ctx, enc, &opts) < 0) {
        avcodec_free_context(&ctx);
        avformat_free_context(oc);
        return nil;
    }
    if (avcodec_parameters_from_context(stream->codecpar, ctx) < 0) {
        avcodec_free_context(&ctx);
        avformat_free_context(oc);
        return nil;
    }

    struct VectorCallback {
        std::vector<uint8_t>* out;
    };
    static const int BUFFER_SIZE = 32768;
    unsigned char* iobuffer = static_cast<unsigned char*>(av_malloc(BUFFER_SIZE));
    if (!iobuffer) {
        avcodec_free_context(&ctx);
        avformat_free_context(oc);
        return nil;
    }

    VectorCallback cb;
    std::vector<uint8_t> output;
    cb.out = &output;

    AVIOContext* write_io = nullptr;
    auto write_cb = [](void* opaque, const uint8_t* buf, int size) -> int {
        VectorCallback* c = static_cast<VectorCallback*>(opaque);
        if (!c || !c->out) {
            return AVERROR_INVALIDDATA;
        }
        c->out->insert(c->out->end(), buf, buf + size);
        return size;
    };

    write_io = avio_alloc_context(iobuffer, BUFFER_SIZE, 1, &cb, nullptr, write_cb, nullptr);
    if (!write_io) {
        av_freep(&iobuffer);
        avcodec_free_context(&ctx);
        avformat_free_context(oc);
        return nil;
    }
    oc->pb = write_io;
    oc->flags |= AVFMT_FLAG_CUSTOM_IO;

    if (avformat_write_header(oc, &opts) < 0) {
        if (write_io) {
            av_freep(&write_io->buffer);
            avio_context_free(&write_io);
        }
        avcodec_free_context(&ctx);
        avformat_free_context(oc);
        return nil;
    }

    AVPacket* pkt = av_packet_alloc();
    bool had_error = false;
    for (size_t i = 0; i < selected.size(); i++) {
        AVFrame* frame = av_frame_alloc();
        if (!frame) {
            had_error = true;
            break;
        }
        frame->format = AV_PIX_FMT_RGB8;
        frame->width = w;
        frame->height = h;
        if (av_frame_get_buffer(frame, 32) < 0) {
            had_error = true;
            av_frame_free(&frame);
            break;
        }
        std::memcpy(frame->data[0], selected[i].rgb.data(), selected[i].rgb.size());
        frame->pts = static_cast<int64_t>(i);
        frame->time_base = stream->time_base;

        if (avcodec_send_frame(ctx, frame) < 0) {
            av_frame_free(&frame);
            had_error = true;
            break;
        }
        while (true) {
            int r = avcodec_receive_packet(ctx, pkt);
            if (r == AVERROR(EAGAIN) || r == AVERROR_EOF) {
                break;
            }
            if (r < 0) {
                had_error = true;
                break;
            }
            pkt->stream_index = stream->index;
            av_packet_rescale_ts(pkt, ctx->time_base, stream->time_base);
            if (av_interleaved_write_frame(oc, pkt) < 0) {
                had_error = true;
            }
            av_packet_unref(pkt);
        }
        av_frame_free(&frame);
        if (had_error) {
            break;
        }
    }

    if (!had_error) {
        if (avcodec_send_frame(ctx, nullptr) >= 0) {
            while (avcodec_receive_packet(ctx, pkt) >= 0) {
                pkt->stream_index = stream->index;
                av_packet_rescale_ts(pkt, ctx->time_base, stream->time_base);
                if (av_interleaved_write_frame(oc, pkt) < 0) {
                    break;
                }
                av_packet_unref(pkt);
            }
        }
        av_write_trailer(oc);
    }

    av_packet_free(&pkt);
    avcodec_free_context(&ctx);
    if (write_io) {
        av_freep(&write_io->buffer);
        avio_context_free(&write_io);
    }
    avformat_free_context(oc);

    if (had_error || output.empty()) {
        return nil;
    }

    uint8_t* result = static_cast<uint8_t*>(std::malloc(output.size()));
    if (!result) {
        return nil;
    }
    std::memcpy(result, output.data(), output.size());
    imgbuf out;
    out.data = result;
    out.len = output.size();
    return out;
}