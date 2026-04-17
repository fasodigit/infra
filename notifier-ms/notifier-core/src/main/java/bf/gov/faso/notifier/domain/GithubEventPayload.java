/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.domain;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.List;

/**
 * GithubEventPayload — JSON deserialization model for events produced by
 * ARMAGEDDON's webhook handler on topic {@code github.events.v1}.
 *
 * <p>Covers push events, pull_request events, and a raw fallback.
 * The {@code eventType} discriminator drives template selection.
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public record GithubEventPayload(

    @JsonProperty("event_type")       String eventType,
    @JsonProperty("delivery_id")      String deliveryId,
    @JsonProperty("repository")       Repository repository,
    @JsonProperty("sender")           Sender sender,
    @JsonProperty("ref")              String ref,
    @JsonProperty("compare")          String compareUrl,
    @JsonProperty("commits")          List<Commit> commits,
    @JsonProperty("pull_request")     PullRequest pullRequest
) {

    @JsonIgnoreProperties(ignoreUnknown = true)
    public record Repository(
        @JsonProperty("full_name")    String fullName,
        @JsonProperty("name")         String name,
        @JsonProperty("html_url")     String htmlUrl,
        @JsonProperty("description")  String description
    ) {}

    @JsonIgnoreProperties(ignoreUnknown = true)
    public record Sender(
        @JsonProperty("login")        String login,
        @JsonProperty("avatar_url")   String avatarUrl,
        @JsonProperty("html_url")     String htmlUrl
    ) {}

    @JsonIgnoreProperties(ignoreUnknown = true)
    public record Commit(
        @JsonProperty("id")           String id,
        @JsonProperty("message")      String message,
        @JsonProperty("author")       CommitAuthor author,
        @JsonProperty("url")          String url,
        @JsonProperty("added")        List<String> added,
        @JsonProperty("modified")     List<String> modified,
        @JsonProperty("removed")      List<String> removed
    ) {}

    @JsonIgnoreProperties(ignoreUnknown = true)
    public record CommitAuthor(
        @JsonProperty("name")         String name,
        @JsonProperty("email")        String email
    ) {}

    @JsonIgnoreProperties(ignoreUnknown = true)
    public record PullRequest(
        @JsonProperty("number")       Integer number,
        @JsonProperty("title")        String title,
        @JsonProperty("html_url")     String htmlUrl,
        @JsonProperty("state")        String state,
        @JsonProperty("merged")       Boolean merged,
        @JsonProperty("body")         String body,
        @JsonProperty("user")         Sender user,
        @JsonProperty("head")         Branch head,
        @JsonProperty("base")         Branch base
    ) {}

    @JsonIgnoreProperties(ignoreUnknown = true)
    public record Branch(
        @JsonProperty("ref")          String ref,
        @JsonProperty("sha")          String sha
    ) {}
}
