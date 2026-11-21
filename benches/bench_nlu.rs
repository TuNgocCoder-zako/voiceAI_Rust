use criterion::{black_box, criterion_group, criterion_main, Criterion};
use voice_ai::core::nlu::fast_path_nlu;
use voice_ai::tools::apps::build_app_index;

fn bench_fast_path(c: &mut Criterion) {
    let app_index = build_app_index();
    let commands = [
        "tăng âm lượng 20%",
        "giảm âm lượng",
        "tắt tiếng",
        "bật tiếng",
        "tăng độ sáng màn hình",
        "bật wifi",
        "pin còn bao nhiêu",
        "bây giờ là mấy giờ",
        "hôm nay ngày mấy",
        "mở visual studio code",
        "đóng cửa sổ",
        "kiểm tra trạng thái git",
        "hẹn giờ 5 phút",
    ];

    c.bench_function("fast_path_nlu_batch", |b| {
        b.iter(|| {
            for &cmd in &commands {
                let _ = fast_path_nlu(black_box(cmd), black_box(&app_index), black_box(true));
            }
        });
    });
}

criterion_group!(benches, bench_fast_path);
criterion_main!(benches);
