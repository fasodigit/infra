package bf.gov.faso.impression.entity;

import io.hypersistence.utils.hibernate.type.json.JsonBinaryType;
import jakarta.persistence.*;
import org.hibernate.annotations.Type;
import org.springframework.data.annotation.CreatedDate;
import org.springframework.data.jpa.domain.support.AuditingEntityListener;

import java.time.Instant;
import java.util.HashMap;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;

/**
 * Entity representing a document delivery record.
 *
 * Tracks all deliveries of printed documents to clients, including
 * signature capture and delivery method.
 */
@Entity
@Table(
    name = "delivery_records",
    indexes = {
        @Index(name = "idx_delivery_print_job_id", columnList = "print_job_id"),
        @Index(name = "idx_delivery_tenant_id", columnList = "tenant_id"),
        @Index(name = "idx_delivery_client_id", columnList = "client_id"),
        @Index(name = "idx_delivery_delivered_at", columnList = "delivered_at")
    }
)
@EntityListeners(AuditingEntityListener.class)
public class DeliveryRecord {

    @Id
    @Column(name = "id", updatable = false, nullable = false)
    private UUID id;

    @Column(name = "print_job_id", nullable = false)
    private UUID printJobId;

    @Column(name = "document_id", nullable = false)
    private UUID documentId;

    @Column(name = "tenant_id", nullable = false)
    private String tenantId;

    @Column(name = "client_id", nullable = false)
    private UUID clientId;

    @Column(name = "operator_id", nullable = false)
    private UUID operatorId;

    @Enumerated(EnumType.STRING)
    @Column(name = "delivery_method", nullable = false, length = 30)
    private DeliveryMethod deliveryMethod;

    @Column(name = "recipient_name", nullable = false, length = 255)
    private String recipientName;

    @Column(name = "recipient_id_number", length = 50)
    private String recipientIdNumber;

    @Column(name = "recipient_id_type", length = 50)
    private String recipientIdType;

    @Column(name = "recipient_phone", length = 20)
    private String recipientPhone;

    @Column(name = "recipient_email", length = 255)
    private String recipientEmail;

    @Column(name = "recipient_relationship", length = 100)
    private String recipientRelationship;

    @Column(name = "signature_data", columnDefinition = "TEXT")
    private String signatureData;

    @Column(name = "signature_hash", length = 128)
    private String signatureHash;

    @Column(name = "delivery_location", length = 500)
    private String deliveryLocation;

    @Column(name = "tracking_number", length = 100)
    private String trackingNumber;

    @Column(name = "courier_name", length = 100)
    private String courierName;

    @Type(JsonBinaryType.class)
    @Column(name = "metadata", columnDefinition = "jsonb")
    private Map<String, Object> metadata = new HashMap<>();

    @Column(name = "notes", length = 2000)
    private String notes;

    @CreatedDate
    @Column(name = "delivered_at", nullable = false, updatable = false)
    private Instant deliveredAt;

    // Constructors
    public DeliveryRecord() {
        this.id = UUID.randomUUID();
    }

    public DeliveryRecord(UUID printJobId, UUID documentId, String tenantId,
                          UUID clientId, UUID operatorId, DeliveryMethod deliveryMethod) {
        this();
        this.printJobId = printJobId;
        this.documentId = documentId;
        this.tenantId = tenantId;
        this.clientId = clientId;
        this.operatorId = operatorId;
        this.deliveryMethod = deliveryMethod;
    }

    // Getters and Setters
    public UUID getId() {
        return id;
    }

    public void setId(UUID id) {
        this.id = id;
    }

    public UUID getPrintJobId() {
        return printJobId;
    }

    public void setPrintJobId(UUID printJobId) {
        this.printJobId = printJobId;
    }

    public UUID getDocumentId() {
        return documentId;
    }

    public void setDocumentId(UUID documentId) {
        this.documentId = documentId;
    }

    public String getTenantId() {
        return tenantId;
    }

    public void setTenantId(String tenantId) {
        this.tenantId = tenantId;
    }

    public UUID getClientId() {
        return clientId;
    }

    public void setClientId(UUID clientId) {
        this.clientId = clientId;
    }

    public UUID getOperatorId() {
        return operatorId;
    }

    public void setOperatorId(UUID operatorId) {
        this.operatorId = operatorId;
    }

    public DeliveryMethod getDeliveryMethod() {
        return deliveryMethod;
    }

    public void setDeliveryMethod(DeliveryMethod deliveryMethod) {
        this.deliveryMethod = deliveryMethod;
    }

    public String getRecipientName() {
        return recipientName;
    }

    public void setRecipientName(String recipientName) {
        this.recipientName = recipientName;
    }

    public String getRecipientIdNumber() {
        return recipientIdNumber;
    }

    public void setRecipientIdNumber(String recipientIdNumber) {
        this.recipientIdNumber = recipientIdNumber;
    }

    public String getRecipientIdType() {
        return recipientIdType;
    }

    public void setRecipientIdType(String recipientIdType) {
        this.recipientIdType = recipientIdType;
    }

    public String getRecipientPhone() {
        return recipientPhone;
    }

    public void setRecipientPhone(String recipientPhone) {
        this.recipientPhone = recipientPhone;
    }

    public String getRecipientEmail() {
        return recipientEmail;
    }

    public void setRecipientEmail(String recipientEmail) {
        this.recipientEmail = recipientEmail;
    }

    public String getRecipientRelationship() {
        return recipientRelationship;
    }

    public void setRecipientRelationship(String recipientRelationship) {
        this.recipientRelationship = recipientRelationship;
    }

    public String getSignatureData() {
        return signatureData;
    }

    public void setSignatureData(String signatureData) {
        this.signatureData = signatureData;
    }

    public String getSignatureHash() {
        return signatureHash;
    }

    public void setSignatureHash(String signatureHash) {
        this.signatureHash = signatureHash;
    }

    public String getDeliveryLocation() {
        return deliveryLocation;
    }

    public void setDeliveryLocation(String deliveryLocation) {
        this.deliveryLocation = deliveryLocation;
    }

    public String getTrackingNumber() {
        return trackingNumber;
    }

    public void setTrackingNumber(String trackingNumber) {
        this.trackingNumber = trackingNumber;
    }

    public String getCourierName() {
        return courierName;
    }

    public void setCourierName(String courierName) {
        this.courierName = courierName;
    }

    public Map<String, Object> getMetadata() {
        return metadata;
    }

    public void setMetadata(Map<String, Object> metadata) {
        this.metadata = metadata;
    }

    public String getNotes() {
        return notes;
    }

    public void setNotes(String notes) {
        this.notes = notes;
    }

    public Instant getDeliveredAt() {
        return deliveredAt;
    }

    public void setDeliveredAt(Instant deliveredAt) {
        this.deliveredAt = deliveredAt;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        DeliveryRecord that = (DeliveryRecord) o;
        return Objects.equals(id, that.id);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id);
    }

    @Override
    public String toString() {
        return "DeliveryRecord{" +
                "id=" + id +
                ", printJobId=" + printJobId +
                ", recipientName='" + recipientName + '\'' +
                ", deliveryMethod=" + deliveryMethod +
                ", deliveredAt=" + deliveredAt +
                '}';
    }
}
