package bf.gov.faso.impression.exception;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.http.ProblemDetail;
import org.springframework.security.access.AccessDeniedException;
import org.springframework.web.bind.MethodArgumentNotValidException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.net.URI;
import java.time.Instant;
import java.util.stream.Collectors;

/**
 * Global exception handler using RFC 7807 Problem Details.
 */
@RestControllerAdvice
public class GlobalExceptionHandler {

    private static final Logger log = LoggerFactory.getLogger(GlobalExceptionHandler.class);

    private static final String PROBLEM_BASE_URI = "https://api.actes.gov.bf/problems/";

    @ExceptionHandler(PrintJobNotFoundException.class)
    public ProblemDetail handlePrintJobNotFound(PrintJobNotFoundException ex) {
        log.warn("Print job not found: {}", ex.getPrintJobId());

        ProblemDetail problem = ProblemDetail.forStatusAndDetail(
            HttpStatus.NOT_FOUND, ex.getMessage());
        problem.setType(URI.create(PROBLEM_BASE_URI + "print-job-not-found"));
        problem.setTitle("Print Job Not Found");
        problem.setProperty("printJobId", ex.getPrintJobId());
        problem.setProperty("timestamp", Instant.now());

        return problem;
    }

    @ExceptionHandler(WormViolationException.class)
    public ProblemDetail handleWormViolation(WormViolationException ex) {
        log.error("WORM violation: {}", ex.getMessage());

        ProblemDetail problem = ProblemDetail.forStatusAndDetail(
            HttpStatus.CONFLICT, ex.getMessage());
        problem.setType(URI.create(PROBLEM_BASE_URI + "worm-violation"));
        problem.setTitle("WORM Storage Violation");
        problem.setProperty("documentId", ex.getDocumentId());
        problem.setProperty("operation", ex.getOperation());
        problem.setProperty("timestamp", Instant.now());

        return problem;
    }

    @ExceptionHandler(InvalidPrintStateException.class)
    public ProblemDetail handleInvalidPrintState(InvalidPrintStateException ex) {
        log.warn("Invalid print state: {}", ex.getMessage());

        ProblemDetail problem = ProblemDetail.forStatusAndDetail(
            HttpStatus.CONFLICT, ex.getMessage());
        problem.setType(URI.create(PROBLEM_BASE_URI + "invalid-print-state"));
        problem.setTitle("Invalid Print Job State");
        problem.setProperty("printJobId", ex.getPrintJobId());
        problem.setProperty("currentStatus", ex.getCurrentStatus());
        problem.setProperty("expectedStatus", ex.getExpectedStatus());
        problem.setProperty("timestamp", Instant.now());

        return problem;
    }

    @ExceptionHandler(BlockchainIntegrityException.class)
    public ProblemDetail handleBlockchainIntegrity(BlockchainIntegrityException ex) {
        log.error("Blockchain integrity failure: {}", ex.getMessage());

        ProblemDetail problem = ProblemDetail.forStatusAndDetail(
            HttpStatus.INTERNAL_SERVER_ERROR, ex.getMessage());
        problem.setType(URI.create(PROBLEM_BASE_URI + "blockchain-integrity-failure"));
        problem.setTitle("Blockchain Integrity Failure");
        problem.setProperty("blockHash", ex.getBlockHash());
        problem.setProperty("timestamp", Instant.now());

        return problem;
    }

    @ExceptionHandler(ReprintNotAuthorizedException.class)
    public ProblemDetail handleReprintNotAuthorized(ReprintNotAuthorizedException ex) {
        log.warn("Reprint not authorized: {}", ex.getMessage());

        ProblemDetail problem = ProblemDetail.forStatusAndDetail(
            HttpStatus.FORBIDDEN, ex.getMessage());
        problem.setType(URI.create(PROBLEM_BASE_URI + "reprint-not-authorized"));
        problem.setTitle("Reprint Not Authorized");
        problem.setProperty("printJobId", ex.getPrintJobId());
        problem.setProperty("wormLocked", ex.isWormLocked());
        problem.setProperty("timestamp", Instant.now());

        return problem;
    }

    @ExceptionHandler(AccessDeniedException.class)
    public ProblemDetail handleAccessDenied(AccessDeniedException ex) {
        log.warn("Access denied: {}", ex.getMessage());

        ProblemDetail problem = ProblemDetail.forStatusAndDetail(
            HttpStatus.FORBIDDEN, "You do not have permission to perform this operation");
        problem.setType(URI.create(PROBLEM_BASE_URI + "access-denied"));
        problem.setTitle("Access Denied");
        problem.setProperty("timestamp", Instant.now());

        return problem;
    }

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ProblemDetail handleValidation(MethodArgumentNotValidException ex) {
        String errors = ex.getBindingResult().getFieldErrors().stream()
            .map(error -> error.getField() + ": " + error.getDefaultMessage())
            .collect(Collectors.joining(", "));

        log.warn("Validation failed: {}", errors);

        ProblemDetail problem = ProblemDetail.forStatusAndDetail(
            HttpStatus.BAD_REQUEST, "Validation failed: " + errors);
        problem.setType(URI.create(PROBLEM_BASE_URI + "validation-error"));
        problem.setTitle("Validation Error");
        problem.setProperty("errors", ex.getBindingResult().getFieldErrors().stream()
            .map(error -> new ValidationError(error.getField(), error.getDefaultMessage()))
            .toList());
        problem.setProperty("timestamp", Instant.now());

        return problem;
    }

    @ExceptionHandler(Exception.class)
    public ProblemDetail handleGeneric(Exception ex) {
        log.error("Unexpected error", ex);

        ProblemDetail problem = ProblemDetail.forStatusAndDetail(
            HttpStatus.INTERNAL_SERVER_ERROR, "An unexpected error occurred");
        problem.setType(URI.create(PROBLEM_BASE_URI + "internal-error"));
        problem.setTitle("Internal Server Error");
        problem.setProperty("timestamp", Instant.now());

        return problem;
    }

    /**
     * Validation error DTO.
     */
    public record ValidationError(String field, String message) {}
}
