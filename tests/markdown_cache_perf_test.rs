use std::time::Instant;

use discord_rs::chat::markdown::{
    parse_markdown, prewarm_message_markdown_cache, render_message_markdown,
};
use discord_rs::chat::markdown_cache::{
    clear_message_markdown_cache, content_hash, get_or_parse_message_markdown,
    message_markdown_cache_stats, MarkdownCacheKey, MarkdownLruCache,
};

fn sample_markdown(i: usize) -> String {
    format!(
        "**Mensagem {i}** com _markdown_ `inline`, ~~strike~~, [link](https://example.com/{i})\n> quote {i}\n```rs\nfn main() {{ println!(\"{i}\"); }}\n```"
    )
}

#[test]
fn lru_cache_key_includes_message_id_and_content_hash() {
    let mut cache = MarkdownLruCache::with_capacity(8);
    let content_a = "Olá **mundo**";
    let content_b = "Olá **Rust**";

    let first = cache.get_or_parse("msg-1", content_a);
    let second = cache.get_or_parse("msg-1", content_a);
    assert_eq!(first, second);
    assert_eq!(cache.hits(), 1);
    assert_eq!(cache.misses(), 1);
    assert_eq!(cache.inserts(), 1);

    let changed = cache.get_or_parse("msg-1", content_b);
    assert_ne!(changed, first, "content hash change must invalidate cached AST");
    assert_eq!(cache.misses(), 2);

    let other_msg_same_content = cache.get_or_parse("msg-2", content_a);
    assert_eq!(other_msg_same_content, first);
    assert_eq!(cache.misses(), 3, "different message_id is a different cache key");
}

#[test]
fn lru_eviction_is_o1_and_removes_least_recently_used() {
    let mut cache = MarkdownLruCache::with_capacity(2);
    let a = "A **um**";
    let b = "B **dois**";
    let c = "C **três**";

    let _ = cache.get_or_parse("m-a", a);
    let _ = cache.get_or_parse("m-b", b);
    // Touch A so B becomes LRU.
    let _ = cache.get_or_parse("m-a", a);
    let _ = cache.get_or_parse("m-c", c);

    assert_eq!(cache.len(), 2);
    assert!(cache.contains(&MarkdownCacheKey::new("m-a", content_hash(a))));
    assert!(cache.contains(&MarkdownCacheKey::new("m-c", content_hash(c))));
    assert!(
        !cache.contains(&MarkdownCacheKey::new("m-b", content_hash(b))),
        "LRU entry should be evicted"
    );
    assert_eq!(cache.evictions(), 1);
}

#[test]
fn global_message_cache_hit_avoids_reparse() {
    clear_message_markdown_cache();
    let msg_id = "msg-global-perf-1";
    let content = "Cache **O(1)** para `(message_id, content_hash)` com `code` e [link](https://example.com)";

    let miss_ast = get_or_parse_message_markdown(msg_id, content);
    let (len_after_miss, _, hits_after_miss, misses_after_miss, inserts_after_miss, _) =
        message_markdown_cache_stats();
    assert_eq!(len_after_miss, 1);
    assert_eq!(hits_after_miss, 0);
    assert_eq!(misses_after_miss, 1);
    assert_eq!(inserts_after_miss, 1);
    assert_eq!(miss_ast, parse_markdown(content));

    let hit_ast = get_or_parse_message_markdown(msg_id, content);
    let (_, _, hits_after_hit, misses_after_hit, inserts_after_hit, _) =
        message_markdown_cache_stats();
    assert_eq!(hit_ast, miss_ast);
    assert_eq!(hits_after_hit, 1);
    assert_eq!(misses_after_hit, 1);
    assert_eq!(inserts_after_hit, 1, "cache hit must not insert/reparse");

    clear_message_markdown_cache();
}

#[test]
fn prewarm_and_render_use_message_cache() {
    clear_message_markdown_cache();
    let msg_id = "msg-prewarm-1";
    let content = "Olá **Discord** <:rust:123:> ||spoiler||";

    prewarm_message_markdown_cache(msg_id, content);
    let (_, _, hits, misses, inserts, _) = message_markdown_cache_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 1);
    assert_eq!(inserts, 1);

    let _rendered = render_message_markdown(msg_id, content);
    let (_, _, hits2, misses2, inserts2, _) = message_markdown_cache_stats();
    assert_eq!(hits2, 1, "render should hit prewarmed cache");
    assert_eq!(misses2, 1);
    assert_eq!(inserts2, 1);

    clear_message_markdown_cache();
}

#[test]
fn cached_lookup_is_fast_under_120fps_budget() {
    clear_message_markdown_cache();
    let id = "msg-perf-budget";
    let content = sample_markdown(42);
    prewarm_message_markdown_cache(id, &content);

    let iterations = 20_000usize;
    let start = Instant::now();
    for _ in 0..iterations {
        let ast = get_or_parse_message_markdown(id, &content);
        std::hint::black_box(ast.len());
    }
    let elapsed = start.elapsed();
    let per_lookup = elapsed / iterations as u32;

    // 120 FPS frame budget is ~8.33ms. Cached lookup + clone must be well below it.
    assert!(
        per_lookup.as_micros() < 2_000,
        "cached markdown lookup too slow for 120fps: {:?}/lookup ({} iters, total {:?})",
        per_lookup,
        iterations,
        elapsed
    );

    let (_, _, hits, misses, _, _) = message_markdown_cache_stats();
    assert!(hits >= iterations as u64);
    assert_eq!(misses, 1);
    clear_message_markdown_cache();
}

#[test]
fn cache_handles_many_messages_without_reparsing_unchanged() {
    let mut cache = MarkdownLruCache::with_capacity(256);
    let messages: Vec<(String, String)> = (0..200)
        .map(|i| (format!("msg-{i}"), sample_markdown(i)))
        .collect();

    for (id, content) in &messages {
        let _ = cache.get_or_parse(id, content);
    }
    let inserts_after_first_pass = cache.inserts();
    assert_eq!(inserts_after_first_pass, 200);
    assert_eq!(cache.misses(), 200);

    for (id, content) in &messages {
        let ast = cache.get_or_parse(id, content);
        std::hint::black_box(ast.len());
    }
    assert_eq!(cache.inserts(), 200, "second pass should be all cache hits");
    assert_eq!(cache.hits(), 200);
}
